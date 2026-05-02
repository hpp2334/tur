use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use parley::LayoutContext as ParleyLayoutContext;
use tur_shared::Constraints;

use crate::core::elements::ElementTree;
use crate::core::event::queue::AppEventQueue;
use crate::core::event::AppEvent;
use crate::core::focus::FocusManager;
use crate::core::fonts::FontManager;
use crate::core::gesture::GestureEventComposer;
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::js_command::JsCommandQueue;
use crate::core::render::Renderer;

pub struct TurAppContext {
    pub(crate) element_tree: Rc<RefCell<ElementTree>>,
    pub(crate) js_command_queue: Rc<RefCell<JsCommandQueue>>,
    pub(crate) focus_manager: Rc<RefCell<FocusManager>>,
    pub(crate) renderer: Box<dyn Renderer>,
    pub(crate) font_manager: FontManager,
    pub(crate) text_layout_cx: ParleyLayoutContext<[u8; 4]>,
    pub(crate) size: (f64, f64),
    pub(crate) gesture_composer: GestureEventComposer,
    pub(crate) event_queue: AppEventQueue,
    pub(crate) handlers: Vec<Box<dyn AppHandler>>,
}

impl fmt::Debug for TurAppContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurAppContext")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl TurAppContext {
    pub fn new(
        element_tree: Rc<RefCell<ElementTree>>,
        js_command_queue: Rc<RefCell<JsCommandQueue>>,
        focus_manager: Rc<RefCell<FocusManager>>,
        renderer: Box<dyn Renderer>,
        font_loader: Box<dyn crate::core::fonts::FontLoader>,
    ) -> Self {
        let font_manager = FontManager::new(font_loader);
        Self {
            element_tree,
            js_command_queue,
            focus_manager,
            renderer,
            font_manager,
            text_layout_cx: ParleyLayoutContext::new(),
            size: (400.0, 600.0),
            gesture_composer: GestureEventComposer::new(),
            event_queue: AppEventQueue::new(),
            handlers: vec![],
        }
    }

    pub fn register_handler(&mut self, handler: Box<dyn AppHandler>) {
        self.handlers.push(handler);
    }

    pub fn dispatch_handlers(&mut self, event: &AppEvent, needs_draw: &Cell<bool>) {
        let mut tree = self.element_tree.borrow_mut();
        let mut focus = self.focus_manager.borrow_mut();
        let mut js_q = self.js_command_queue.borrow_mut();
        let mut cx = HandlerContext {
            element_tree: &mut tree,
            focus_manager: &mut focus,
            js_command_queue: &mut js_q,
            event_queue: &mut self.event_queue,
            gesture_composer: &mut self.gesture_composer,
            renderer: self.renderer.as_mut(),
            size: &mut self.size,
            needs_draw,
        };
        for handler in &mut self.handlers {
            handler.handle_event(&mut cx, event);
        }
    }

    pub fn layout(&mut self) {
        let (width, height) = self.size;
        let constraints = Constraints {
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        };

        let mut tree = self.element_tree.borrow_mut();
        tree.compute_layout(
            &constraints,
            &mut self.font_manager,
            &mut self.text_layout_cx,
        );
    }

    pub fn render(&mut self) {
        let focused_node_id = self.focus_manager.borrow().focused();
        let tree = self.element_tree.borrow();
        self.renderer.render(&tree, focused_node_id);
    }
}
