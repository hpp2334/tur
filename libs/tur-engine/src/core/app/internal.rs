use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::app::TurAppContext;
use crate::core::bridge::TurJsContext;
use crate::core::event::AppEvent;
use crate::core::fonts::FontLoader;
use crate::core::render::Renderer;
use crate::error::TurError;

pub struct TurAppInternal {
    pub(crate) js_context: TurJsContext,
    pub(crate) app_context: Rc<RefCell<TurAppContext>>,
    needs_draw: Cell<bool>,
}

impl TurAppInternal {
    pub fn new(
        renderer: Box<dyn Renderer>,
        font_loader: Box<dyn FontLoader>,
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

        let app_context = TurAppContext::new(
            element_tree,
            js_command_queue,
            focus_manager,
            resource_map,
            renderer,
            font_loader,
        );

        Self {
            js_context,
            app_context: Rc::new(RefCell::new(app_context)),
            needs_draw: Cell::new(false),
        }
    }

    pub fn flush(
        &self,
        boa_context: &mut boa_engine::Context,
    ) -> Result<(), TurError> {
        loop {
            let handled_events = self.flush_app_events();
            let dirty = self.js_context.dirty.take() || self.needs_draw.take();
            if dirty {
                self.app_context.borrow_mut().layout();
            }
            let handled_commands = self.flush_js_commands(boa_context);
            if !handled_events && !handled_commands {
                break;
            }
        }
        self.app_context.borrow_mut().render();
        self.app_context
            .borrow_mut()
            .renderer
            .present()
            .map_err(|e| TurError::Render(e.to_string()))
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
        let mut pending_callbacks: Vec<(boa_engine::object::JsObject, Vec<boa_engine::JsValue>)> =
            Vec::new();

        loop {
            let entries = self.js_context.js_command_queue.borrow_mut().drain();
            if entries.is_empty() {
                break;
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
        }

        let handled = !pending_callbacks.is_empty();

        for (callback, args) in pending_callbacks {
            let _ = callback.call(
                &boa_engine::JsValue::undefined(),
                &args,
                boa_context,
            );
        }

        handled
    }

}
