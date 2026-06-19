use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::app::TurAppContext;
use crate::core::bridge::TurJobExecutor;
use crate::core::bridge::TurJsContext;
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
    ) -> Self {
        use crate::core::elements::ElementTree;
        use crate::core::edgy_event::PendingMutationInvocationQueue;
        use crate::core::focus::FocusManager;
        use crate::core::resource::ResourceMap;

        let element_tree = Rc::new(RefCell::new(ElementTree::new()));
        let mutation_queue = Rc::new(RefCell::new(PendingMutationInvocationQueue::new()));
        let focus_manager = Rc::new(RefCell::new(FocusManager::new()));
        let dirty = Rc::new(Cell::new(false));
        let resource_map = Rc::new(RefCell::new(ResourceMap::default()));

        let js_context = TurJsContext::new(
            element_tree.clone(),
            mutation_queue.clone(),
            focus_manager.clone(),
            dirty,
            resource_map.clone(),
        );

        // Give the tree access to the reactive store so layout/paint can
        // resolve `Val<T>` values on demand.
        element_tree.borrow_mut().set_store(js_context.store.clone());

        let app_context = TurAppContext::new(
            element_tree,
            mutation_queue,
            focus_manager,
            resource_map,
            renderer,
            font_loader,
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
            let reactive_changed = self.flush_reactive(boa_context);

            let dirty =
                self.js_context.dirty.take() || self.needs_draw.take() || animation_did_update || reactive_changed;
            if dirty {
                needs_render = true;
                self.app_context.borrow_mut().layout();
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

        if needs_render {
            self.app_context.borrow_mut().render();
            if let Err(e) = self.app_context.borrow_mut().renderer.present() {
                tracing::error!("present failed: {e}");
                return Err(TurError::Render(e.to_string()));
            }
        }
        Ok(needs_render)
    }

    /// Drain the reactive store and mark affected tree nodes dirty via the
    /// dep tracker. Returns `true` if any nodes were dirtied.
    fn flush_reactive(&self, boa_context: &mut boa_engine::Context) -> bool {
        let store = self.js_context.store.clone();
        if !store.borrow().has_pending() {
            return false;
        }
        let store_ctx_obj = match crate::core::bridge::reactive_bridge::build_ctx_object_for(
            store.clone(),
            boa_context,
        ) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("reactive store-ctx build failed: {e}");
                return false;
            }
        };
        let dirties = store.borrow().flush(boa_context, &store_ctx_obj);
        if dirties.is_empty() {
            return false;
        }
        // Mark nodes whose atoms are dirty, so the next layout pass
        // re-reads fresh values via `LayoutContext::read_val`.
        let dirty_nodes = self
            .js_context
            .element_tree
            .borrow()
            .dep_tracker()
            .dirty_nodes(&dirties);
        let mut tree = self.js_context.element_tree.borrow_mut();
        for node_id in &dirty_nodes {
            tree.mark_dirty(*node_id);
        }
        drop(tree);

        // Run effects (Condition branch swaps, LazyList range adjustments).
        self.run_effects(boa_context, &dirties);

        !dirty_nodes.is_empty()
    }

    /// Walk all tree nodes and invoke `run_effect` — lets Condition /
    /// LazyList react to dirty atoms before layout.
    fn run_effects(
        &self,
        boa_context: &mut boa_engine::Context,
        dirties: &std::collections::HashSet<crate::core::reactive::AtomId>,
    ) {
        let node_ids: Vec<crate::core::element::ElementNodeId> = {
            let tree = self.js_context.element_tree.borrow();
            tree.nodes.keys().copied().collect()
        };
        let mut cx = crate::core::widget::WidgetCx::new(self.js_context.clone());
        for id in node_ids {
            let mut element = {
                let mut tree = self.js_context.element_tree.borrow_mut();
                tree.get_mut(id).and_then(|n| n.element.take())
            };
            if let Some(ref mut elem) = element {
                elem.run_effect(&mut cx, boa_context, dirties);
            }
            if let Some(elem) = element {
                let mut tree = self.js_context.element_tree.borrow_mut();
                if let Some(node) = tree.get_mut(id) {
                    node.element = Some(elem);
                }
            }
        }
    }

    fn tick_animations(&self, boa_context: &mut boa_engine::Context) -> bool {
        let now_ms = boa_context.clock().now().millis_since_epoch();
        let mut mgr = self.js_context.animation_manager.borrow_mut();
        mgr.tick_controllers(now_ms, boa_context);
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
        let ctx_obj = crate::core::reactive::build_store_context_object(boa_context, store.clone())
            .ok()
            .map(boa_engine::JsValue::from);
        for inv in invs {
            let mut args: Vec<boa_engine::JsValue> = Vec::new();
            if let Some(o) = &ctx_obj {
                args.push(o.clone());
            }
            args.extend(inv.args.to_js_args(boa_context));
            let _ = store.borrow().invoke_mutation(inv.atom_id, &args, boa_context);
        }
        true
    }
}

fn focus_mutation(
    tree: &crate::core::elements::ElementTree,
    id: crate::core::element::ElementNodeId,
) -> Option<crate::core::edgy_event::EdgyMutation<crate::core::focus::FocusEvent>> {
    use crate::elements::{EditableTextElement, FocusableElement};
    let node = tree.get(id)?;
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
    let node = tree.get(id)?;
    let element = node.element.as_ref()?;
    if let Some(f) = element.cast::<FocusableElement>() {
        return f.component.on_blur;
    }
    if let Some(e) = element.cast::<EditableTextElement>() {
        return e.controller().on_blur();
    }
    None
}
