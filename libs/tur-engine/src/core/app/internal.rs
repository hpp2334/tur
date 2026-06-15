use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::app::TurAppContext;
use crate::core::bridge::TurJobExecutor;
use crate::core::bridge::TurJsContext;
use crate::core::event::AppEvent;
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
        use crate::core::focus::FocusManager;
        use crate::core::js_command::JsCommandQueue;
        use crate::core::resource::ResourceMap;

        let element_tree = Rc::new(RefCell::new(ElementTree::new()));
        let js_command_queue = Rc::new(RefCell::new(JsCommandQueue::new()));
        let focus_manager = Rc::new(RefCell::new(FocusManager::new()));
        let dirty = Rc::new(Cell::new(false));
        let resource_map = Rc::new(RefCell::new(ResourceMap::default()));

        let js_context = TurJsContext::new(
            element_tree.clone(),
            js_command_queue.clone(),
            focus_manager.clone(),
            dirty,
            resource_map.clone(),
        );

        // Give the tree access to the reactive store so layout/paint can
        // resolve `Val<T>` values on demand.
        element_tree.borrow_mut().set_store(js_context.store.clone());

        let app_context = TurAppContext::new(
            element_tree,
            js_command_queue,
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
            let handled_commands = self.flush_js_commands(boa_context);
            let _ = self.executor.drain(boa_context);
            let new_dirty = self.js_context.dirty.get() || self.needs_draw.get();
            if !handled_events && !handled_commands && !new_dirty {
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

    fn flush_js_commands(
        &self,
        boa_context: &mut boa_engine::Context,
    ) -> bool {
        let mut pending_callbacks: Vec<(
            boa_engine::object::builtins::JsFunction,
            Vec<boa_engine::JsValue>,
        )> = Vec::new();

        let entries = self.js_context.js_command_queue.borrow_mut().drain();
        if entries.is_empty() {
            return false;
        }

        for (target, command) in entries {
            let tree = self.js_context.element_tree.borrow();
            if let Some(node) = tree.get(target) {
                if let Some(ref element) = node.element {
                    if let Some(pair) = element.emit_js_callback(boa_context, command) {
                        pending_callbacks.push(pair);
                    }
                }
            }
        }

        let handled = !pending_callbacks.is_empty();

        for (callback, args) in pending_callbacks {
            let _ = callback.call(&boa_engine::JsValue::undefined(), &args, boa_context);
        }

        handled
    }
}
