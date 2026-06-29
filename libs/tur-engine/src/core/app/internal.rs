use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use boa_engine::context::time::FixedClock;

use crate::core::app::TurAppContext;
use crate::core::bridge::TurJobExecutor;
use crate::core::bridge::TurJsContext;
use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::event::AppEvent;
use crate::core::focus::{FocusChange, BlurEvent, FocusEvent};
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
        host_api: Box<dyn crate::core::host_api::HostApi>,
    ) -> Self {
        use crate::core::elements::ElementTree;
        use crate::core::edgy_event::PendingMutationInvocationQueue;
        use crate::core::focus::FocusManager;
        use crate::core::reactive::Store;
        use crate::core::resource::ResourceMap;

        let mutation_queue = Rc::new(RefCell::new(PendingMutationInvocationQueue::new()));
        let focus_manager = Rc::new(RefCell::new(FocusManager::new()));
        let dirty = Rc::new(Cell::new(false));
        let resource_map = Rc::new(RefCell::new(ResourceMap::default()));

        let store = Store::new(dirty.clone());
        let element_tree = Rc::new(RefCell::new(ElementTree::new(store.clone())));

        let js_context = TurJsContext::new(
            element_tree.clone(),
            mutation_queue.clone(),
            focus_manager.clone(),
            dirty,
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
            host_api,
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

            // Process any pending LazyList remounts (set by wheel handlers
            // when the visible range shifts). Must run before layout so the
            // newly-mounted children get measured in this pass.
            let remounted = self.process_lazy_remounts(boa_context);

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
            let reactive_changed = self.flush_reactive(boa_context);

            let dirty =
                self.js_context.dirty.take() || self.needs_draw.take() || animation_did_update || reactive_changed || remounted;
            if dirty {
                needs_render = true;
                self.app_context.borrow_mut().layout(boa_context);
            }
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
        use crate::elements::EditableTextElement;
        let Some(focused_id) = self.js_context.focus_manager.borrow().focused() else {
            return false;
        };
        let tree = self.js_context.element_tree.borrow();
        let Some(node) = tree.get_element(focused_id) else {
            return false;
        };
        let Some(ref element) = node.element else {
            return false;
        };
        element.cast::<EditableTextElement>().is_some()
    }

    /// Drain the reactive store and mark affected tree nodes dirty via the
    /// subscriber graph. Returns `true` if any nodes were dirtied.
    fn flush_reactive(&self, boa_context: &mut boa_engine::Context) -> bool {
        let store = self.js_context.store.clone();
        let flush_engine = store.flush_engine();
        if !flush_engine.has_pending() {
            return false;
        }
        let dirties = flush_engine.flush();
        if dirties.is_empty() {
            return false;
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

        // Element effects (LazyList range adjustments, etc.).
        self.run_element_effects(boa_context, &dirties);

        // Fragment rebuilds (Condition / Each / Switch branch swaps).
        self.rebuild_fragments(boa_context, &dirty_frag_ids);

        !dirty_subs.is_empty()
    }

    /// Walk all tree elements and invoke `run_effect` — lets widgets that
    /// implement `Effect` react to dirty atoms before layout (e.g. LazyList
    /// range adjustments).
    fn run_element_effects(
        &self,
        boa_context: &mut boa_engine::Context,
        dirties: &HashSet<crate::core::reactive::AtomId>,
    ) {
        let node_ids: Vec<ElementNodeId> = {
            let tree = self.js_context.element_tree.borrow();
            tree.elements.keys().copied().collect()
        };
        let mut cx = crate::core::widget::WidgetCx::new(self.js_context.clone());
        for id in node_ids {
            let mut element = {
                let mut tree = self.js_context.element_tree.borrow_mut();
                tree.get_element_mut(id).and_then(|n| n.element.take())
            };
            if let Some(ref mut elem) = element {
                elem.run_effect(&mut cx, boa_context, dirties);
            }
            if let Some(elem) = element {
                let mut tree = self.js_context.element_tree.borrow_mut();
                if let Some(node) = tree.get_element_mut(id) {
                    node.element = Some(elem);
                }
            }
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
        let mut cx = crate::core::widget::WidgetCx::new(self.js_context.clone());

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
        mgr.tick_controllers(now_ms, boa_context);
        let has_active = mgr.has_active();
        drop(mgr);
        has_active
    }

    /// Walk the tree and process any `LazyListElement`s whose
    /// `remount_requested` flag is set (typically by `on_wheel` after a
    /// scroll). For each, recompute the visible range based on the current
    /// scroll position + viewport size, mount newly-visible items via the JS
    /// builder, and unmount off-screen ones.
    ///
    /// Returns `true` if any remount happened (so the caller knows to
    /// trigger another layout pass).
    fn process_lazy_remounts(&self, boa_context: &mut boa_engine::Context) -> bool {
        use crate::elements::LazyListElement;

        // Collect candidate node ids first to avoid holding the tree borrow
        // while we mutate. We only need to consider nodes that currently have
        // a LazyListElement with the flag set.
        let candidates: Vec<ElementNodeId> = {
            let tree = self.js_context.element_tree.borrow();
            tree.elements
                .iter()
                .filter_map(|(id, node)| {
                    let el = node.element.as_ref()?;
                    let ll = el.cast::<LazyListElement>()?;
                    if ll.remount_requested { Some(*id) } else { None }
                })
                .collect()
        };
        if candidates.is_empty() {
            return false;
        }

        let mut cx = crate::core::widget::WidgetCx::new(self.js_context.clone());
        let mut any_changed = false;
        for id in candidates {
            // Take the element out of the tree so we can mutate it with
            // exclusive access while still being able to call into the tree
            // (mount/unmount) via `cx`.
            let mut element_opt = {
                let mut tree = self.js_context.element_tree.borrow_mut();
                tree.get_element_mut(id).and_then(|n| n.element.take())
            };
            let Some(mut element) = element_opt.take() else { continue };

            // Read the current viewport size from the node's computed layout.
            let viewport_main = {
                let tree = self.js_context.element_tree.borrow();
                let axis = element
                    .cast::<LazyListElement>()
                    .map(|ll| ll.axis())
                    .unwrap_or(tur_shared::Axis::Vertical);
                tree.get_element(id)
                    .map(|n| match axis {
                        tur_shared::Axis::Vertical => n.computed_layout.size.height,
                        tur_shared::Axis::Horizontal => n.computed_layout.size.width,
                    })
                    .unwrap_or(0.0)
            };

            if let Some(ll) = element.cast_mut::<LazyListElement>() {
                let prev_count = ll.built_count();
                ll.process_remount(&mut cx, boa_context, viewport_main);
                if ll.built_count() != prev_count {
                    any_changed = true;
                }
            }

            // Put the element back.
            let mut tree = self.js_context.element_tree.borrow_mut();
            if let Some(node) = tree.get_element_mut(id) {
                node.element = Some(element);
            }
        }
        any_changed
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
    /// Each pending id is looked up in the element tree; if it resolves to a
    /// focusable element (FocusableElement or EditableTextElement) with an `on_focus` /
    /// `on_blur` mutation, the invocation is pushed onto the pending-mutation
    /// queue. Runs before `flush_pending_mutations` so focus callbacks fire in
    /// the same pass.
    fn flush_focus_notifications(&self) {
        let changes = self.js_context.focus_manager.borrow_mut().drain_pending();
        if changes.is_empty() {
            return;
        }
        let tree = self.js_context.element_tree.borrow();
        let mut queue = self.js_context.mutation_queue.borrow_mut();
        for change in changes {
            match change {
                FocusChange::Focus(id) => {
                    if let Some(m) = focus_mutation(&tree, id) {
                        queue.push(m, FocusEvent);
                    }
                }
                FocusChange::Blur(id) => {
                    if let Some(m) = blur_mutation(&tree, id) {
                        queue.push(m, BlurEvent);
                    }
                }
            }
        }
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

fn focus_mutation(
    tree: &crate::core::elements::ElementTree,
    id: crate::core::element::ElementNodeId,
) -> Option<crate::core::edgy_event::EdgyMutation<crate::core::focus::FocusEvent>> {
    use crate::elements::{EditableTextElement, FocusableElement};
    let node = tree.get_element(id)?;
    let element = node.element.as_ref()?;
    if let Some(f) = element.cast::<FocusableElement>() {
        return f.component.on_focus;
    }
    if let Some(e) = element.cast::<EditableTextElement>() {
        return e.controller().on_focus();
    }
    None
}

fn blur_mutation(
    tree: &crate::core::elements::ElementTree,
    id: crate::core::element::ElementNodeId,
) -> Option<crate::core::edgy_event::EdgyMutation<crate::core::focus::BlurEvent>> {
    use crate::elements::{EditableTextElement, FocusableElement};
    let node = tree.get_element(id)?;
    let element = node.element.as_ref()?;
    if let Some(f) = element.cast::<FocusableElement>() {
        return f.component.on_blur;
    }
    if let Some(e) = element.cast::<EditableTextElement>() {
        return e.controller().on_blur();
    }
    None
}
