use std::cell::RefCell;
use std::rc::Rc;

use crate::core::app::TurAppContext;
use crate::core::bridge::TurJsContext;
use crate::core::render::Renderer;
use crate::core::fonts::FontLoader;

pub struct TurAppInternal {
    pub(crate) js_context: TurJsContext,
    pub(crate) app_context: Rc<RefCell<TurAppContext>>,
}

impl TurAppInternal {
    pub fn new(
        renderer: Box<dyn Renderer>,
        font_loader: Box<dyn FontLoader>,
    ) -> Self {
        use crate::core::elements::ElementTree;
        use crate::core::focus::FocusManager;
        use crate::core::js_command::JsCommandQueue;
        use std::cell::Cell;

        let element_tree = Rc::new(RefCell::new(ElementTree::new()));
        let js_command_queue = Rc::new(RefCell::new(JsCommandQueue::new()));
        let focus_manager = Rc::new(RefCell::new(FocusManager::new()));
        let dirty = Rc::new(Cell::new(false));

        let js_context = TurJsContext::new(
            element_tree.clone(),
            js_command_queue.clone(),
            focus_manager.clone(),
            dirty,
        );

        let app_context = TurAppContext::new(
            element_tree,
            js_command_queue,
            focus_manager,
            renderer,
            font_loader,
        );

        Self {
            js_context,
            app_context: Rc::new(RefCell::new(app_context)),
        }
    }
}
