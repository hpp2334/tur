use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::context::time::FixedClock;

use crate::core::app::TurAppContext;
use crate::core::bridge::TurJobExecutor;
use crate::core::bridge::TurJsContext;
use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::event::AppEvent;
use crate::core::focus::helper;
use crate::core::fonts::FontLoader;
use crate::core::render::Renderer;
use crate::error::TurError;

pub struct TurAppInternal {
    pub(crate) js_context: TurJsContext,
    pub(crate) app_context: Rc<RefCell<TurAppContext>>,
    pub(crate) needs_draw: Rc<Cell<bool>>,
    pub(crate) executor: Rc<TurJobExecutor>,
}

impl TurAppInternal {
    pub fn new(
        renderer: Box<dyn Renderer>,
        font_loader: Box<dyn FontLoader>,
        executor: Rc<TurJobExecutor>,
        clock: std::rc::Rc<FixedClock>,
        platform_api: Box<dyn crate::core::platform_api::PlatformApi>,
    ) -> Self {
        use crate::core::elements::NodeTree;
        use crate::core::edgy_event::PendingMutationInvocationQueue;
        use crate::core::focus::FocusManager;
        use crate::core::reactive::Store;
        use crate::core::resource::ResourceMap;

        let mutation_queue = Rc::new(RefCell::new(PendingMutationInvocationQueue::new()));
        let focus_manager = Rc::new(RefCell::new(FocusManager::new()));
        let dirty = Rc::new(Cell::new(false));
        let resource_map = Rc::new(RefCell::new(ResourceMap::default()));

        let store = Store::new(dirty.clone());
        let element_tree = NodeTree::new(store.clone());

        let js_context = TurJsContext::new(
            element_tree.clone(),
            mutation_queue.clone(),
            focus_manager.clone(),
            dirty.clone(),
            resource_map.clone(),
            store,
        );

        let app_context = TurAppContext::new(
            element_tree,
            mutation_queue,
            focus_manager,
            resource_map,
            renderer,
            font_loader,
            clock,
            platform_api,
        );

        let needs_draw = Rc::new(Cell::new(false));

        Self {
            js_context,
            app_context: Rc::new(RefCell::new(app_context)),
            needs_draw,
            executor,
        }
    }

    pub fn flush(
        &self,
        boa_context: &mut boa_engine::Context,
    ) -> Result<bool, TurError> {
        let mut needs_render = false;
        let mut animation_ticked = false;

        loop {
            let handled_events = self.flush_app_events();

            let animation_did_update = if !animation_ticked {
                animation_ticked = true;
                self.tick_animations(boa_context)
            } else {
                self.js_context.animation_manager.borrow().has_active()
            };

        // Reactive flush: drain the store, expand dirty atoms, and dispatch
        // `do_update(dirties)` to the mounted edgy root. This may mutate
        // the ElementTree, which sets `dirty`/`needs_draw` for the next
        // layout pass.
        let (reactive_changed, dirty_element_ids) = self.flush_reactive(boa_context);

        // LazyList remount now happens *inside* `perform_layout` (it uses
        // the real viewport from constraints), so there is no separate
        // pre-layout remount pass here.
        let dirty =
            self.js_context.dirty.take() || self.needs_draw.take() || animation_did_update || reactive_changed;
        if dirty {
            needs_render = true;
            self.app_context
                .borrow_mut()
                .layout(self.js_context.dirty.clone(), boa_context);
        }
        // Lifecycle hooks fire after layout: on_mounted for inserted
        // elements, on_updated for dirtied elements, before_destroy for
        // removed elements. Pushed mutations are drained right after.
        self.run_lifecycle_hooks(boa_context, &dirty_element_ids);
        self.flush_focus_notifications();
        let handled_mutations = self.flush_pending_mutations(boa_context);
            let _ = self.executor.drain(boa_context);
            let new_dirty = self.js_context.dirty.get() || self.needs_draw.get();
            if !handled_events && !handled_mutations && !new_dirty {
                break;
            }
        }

        if self
            .js_context
            .animation_manager
            .borrow()
            .has_active()
        {
            self.needs_draw.set(true);
        }

        // Cursor blink: when an EditableText holds focus, keep redrawing on
        // every idle frame so the caret's 530ms blink phase is honoured even
        // when no other state is changing.
        if self.focused_is_editable() {
            needs_render = true;
        }

        if needs_render {
            self.app_context.borrow_mut().render();
            if let Err(e) = self.app_context.borrow_mut().renderer.present() {
                tracing::error!("present failed: {e}");
                return Err(TurError::Render(e.to_string()));
            }
        }
        Ok(needs_render)
    }

    /// True if the currently-focused element is an `EditableTextElement`.
    /// Used by `flush` to keep redrawing on idle frames so the caret blink
    /// animates without an explicit animation controller.
    fn focused_is_editable(&self) -> bool {
        let tree = self.js_context.element_tree.borrow();
        let focus = self.js_context.focus_manager.borrow();
        helper::focused_is_editable(&tree, &focus)
    }

    /// Drain the reactive store and mark affected tree nodes dirty via the
    /// subscriber graph. Returns `(reactive_changed, dirty_element_ids)`:
    /// the element ids whose subscribed atoms changed this flush — used by
    /// the flush loop to fire `on_updated` lifecycle hooks after layout.
    fn flush_reactive(
        &self,
        boa_context: &mut boa_engine::Context,
    ) -> (bool, Vec<ElementNodeId>) {
        let store = self.js_context.store.clone();
        let flush_engine = store.flush_engine();
        if !flush_engine.has_pending() {
            return (false, Vec::new());
        }
        let dirties = flush_engine.flush();
        if dirties.is_empty() {
            return (false, Vec::new());
        }

        let dirty_subs = store.subscriber_index().dirty_subscribers(&dirties);

        // Mark all dirty subscribers dirty. mark_dirty handles fragments by
        // skipping them and marking their real parent element.
        {
            let mut tree = self.js_context.element_tree.borrow_mut();
            for sub_id in &dirty_subs {
                tree.mark_dirty(NodeId::new(sub_id.as_u64()));
            }
        }

        // Split dirty subscribers into elements and fragments so fragment
        // rebuilds only process dirty fragments (not a full scan).
        let dirty_frag_ids: Vec<FragmentNodeId> = {
            let tree = self.js_context.element_tree.borrow();
            dirty_subs
                .iter()
                .filter(|s| tree.is_fragment(NodeId::new(s.as_u64())))
                .map(|s| FragmentNodeId::new(s.as_u64()))
                .collect()
        };
        // Element ids dirtied this flush (for the post-layout `on_updated` pass).
        let dirty_element_ids: Vec<ElementNodeId> = {
            let tree = self.js_context.element_tree.borrow();
            dirty_subs
                .iter()
                .filter(|s| !tree.is_fragment(NodeId::new(s.as_u64())))
                .map(|s| ElementNodeId::new(s.as_u64()))
                .collect()
        };

        // Fragment rebuilds (Condition / Each / Switch branch swaps).
        self.rebuild_fragments(boa_context, &dirty_frag_ids);

        (!dirty_subs.is_empty(), dirty_element_ids)
    }

    /// Fire element lifecycle hooks: `on_mounted` for newly-inserted elements,
    /// `on_updated` for elements whose subscribed atoms were dirtied this
    /// flush, and `before_destroy` for elements removed since the last pass.
    /// All hooks run after layout (so the mutation queue is drained by the
    /// subsequent `flush_pending_mutations`).
    fn run_lifecycle_hooks(
        &self,
        boa_context: &mut boa_engine::Context,
        dirty_element_ids: &[ElementNodeId],
    ) {
        let mut cx = crate::core::view::SharedViewCx::new(self.js_context.clone());

        // on_mounted — freshly-inserted elements.
        let mounted_ids = self
            .js_context
            .element_tree
            .borrow_mut()
            .take_pending_mounted();
        for id in mounted_ids {
            let mut element = {
                let mut tree = self.js_context.element_tree.borrow_mut();
                tree.get_element_mut(id).and_then(|n| n.element.take())
            };
            if let Some(ref mut elem) = element {
                elem.run_on_mounted(&mut cx, boa_context);
            }
            if let Some(elem) = element {
                let mut tree = self.js_context.element_tree.borrow_mut();
                if let Some(node) = tree.get_element_mut(id) {
                    node.element = Some(elem);
                }
            }
        }

        // on_updated — elements dirtied this flush (post-layout).
        for id in dirty_element_ids {
            let mut element = {
                let mut tree = self.js_context.element_tree.borrow_mut();
                tree.get_element_mut(*id).and_then(|n| n.element.take())
            };
            if let Some(ref mut elem) = element {
                elem.run_on_updated(&mut cx, boa_context);
            }
            if let Some(elem) = element {
                let mut tree = self.js_context.element_tree.borrow_mut();
                if let Some(node) = tree.get_element_mut(*id) {
                    node.element = Some(elem);
                }
            }
        }

        // before_destroy — elements removed since the last pass. The element
        // is already detached from the tree (taken out during destroy), so we
        // just fire the hook and let it drop.
        let destroyed = self
            .js_context
            .element_tree
            .borrow_mut()
            .take_pending_destroy();
        for mut elem in destroyed {
            elem.run_before_destroy(&mut cx, boa_context);
        }
    }

    /// Rebuild dirty fragments (Condition / Each / Switch). Only fragments
    /// whose subscribed atoms are dirty are processed — identified via the
    /// subscriber graph, not a full scan. Each fragment's `perform_update`
    /// resolves the current value and swaps the branch/items if changed.
    fn rebuild_fragments(
        &self,
        boa_context: &mut boa_engine::Context,
        dirty_frag_ids: &[FragmentNodeId],
    ) {
        let mut cx = crate::core::view::SharedViewCx::new(self.js_context.clone());

        for fid in dirty_frag_ids {
            let mut kind = {
                let mut tree = self.js_context.element_tree.borrow_mut();
                tree.get_fragment_mut(*fid).and_then(|h| h.kind.take())
            };
            let Some(ref mut k) = kind else { continue };

            // Save old children + parent BEFORE rebuild (perform_update
            // auto-links new children to frag.children via append_child).
            let (old_children, parent) = {
                let tree = self.js_context.element_tree.borrow();
                tree.get_fragment(*fid)
                    .map(|f| (f.children.clone(), f.parent))
                    .unwrap_or((Vec::new(), (*fid).into()))
            };

            let new_children = k.perform_update(&mut cx, boa_context, *fid);

            if let Some(new) = new_children {
                // frag.children now has old + new; replace with just new.
                {
                    let mut tree = self.js_context.element_tree.borrow_mut();
                    if let Some(f) = tree.get_fragment_mut(*fid) {
                        f.children = new;
                    }
                }
                // Destroy old subtrees.
                for child in &old_children {
                    cx.destroy_child(*child);
                }
                cx.mark_dirty(parent);
            }

            // Put kind back.
            if let Some(kind) = kind {
                let mut tree = self.js_context.element_tree.borrow_mut();
                if let Some(host) = tree.get_fragment_mut(*fid) {
                    host.kind = Some(kind);
                }
            }
        }
    }

    fn tick_animations(&self, boa_context: &mut boa_engine::Context) -> bool {
        let now_ms = self.app_context.borrow().shell.now().as_millis() as u64;
        let mut mgr = self.js_context.animation_manager.borrow_mut();
        // Cache the clock so a `retarget` issued later this flush (during the
        // element Effect phase) can stamp a precise driver `start_time`.
        mgr.set_clock(now_ms);
        mgr.tick_controllers(now_ms, boa_context);
        // Native implicit-animation drivers (AnimatedContainer /
        // AnimatedOpacity / AnimatedPositioned). Each tick writes the eased
        // progress into the element's shared cell and marks it dirty.
        let _ = mgr.tick_drivers(
            now_ms,
            &self.js_context.element_tree,
            &self.js_context.dirty,
            &self.js_context.mutation_queue,
        );
        let has_active = mgr.has_active();
        drop(mgr);
        has_active
    }

    fn flush_app_events(&self) -> bool {
        let events = self.app_context.borrow_mut().event_queue.drain();
        if events.is_empty() {
            return false;
        }

        for event in &events {
            if matches!(event, AppEvent::RequestDraw) {
                self.needs_draw.set(true);
            }
            self.app_context
                .borrow_mut()
                .dispatch_handlers(event, &self.needs_draw);
        }

        true
    }

    /// Resolve pending focus/blur notifications recorded by `FocusManager`.
    /// Delegates to the focus domain, which maps each pending id to its
    /// `Focusable` element (if any) and enqueues the `on_focus` / `on_blur`
    /// mutation. Runs before `flush_pending_mutations` so focus callbacks
    /// fire in the same pass.
    fn flush_focus_notifications(&self) {
        let tree = self.js_context.element_tree.borrow();
        let mut focus = self.js_context.focus_manager.borrow_mut();
        let mut queue = self.js_context.mutation_queue.borrow_mut();
        focus.flush_pending(&tree, &mut queue);
    }

    /// Drain the pending-mutation queue and invoke each mutation via the
    /// reactive store, prepending the `{get, set}` context object. No element
    /// tree access is needed: every entry is a self-contained `(AtomId, args)`.
    fn flush_pending_mutations(&self, boa_context: &mut boa_engine::Context) -> bool {
        let invs = self.js_context.mutation_queue.borrow_mut().drain();
        if invs.is_empty() {
            return false;
        }
        let store = self.js_context.store.clone();
        let ctx_obj = store
            .ctx_object(boa_context)
            .ok()
            .map(boa_engine::JsValue::from);
        for inv in invs {
            let mut args: Vec<boa_engine::JsValue> = Vec::new();
            if let Some(o) = &ctx_obj {
                args.push(o.clone());
            }
            args.extend(inv.args.to_js_args(boa_context));
            let _ = store.invoke_mutation(inv.mutation, &args, boa_context);
        }
        true
    }
}
