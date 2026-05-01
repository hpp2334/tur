use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use parley::LayoutContext as ParleyLayoutContext;
use tur_shared::Constraints;

use crate::core::element::ElementNodeId;
use crate::core::elements::{
    ComposedGestureEvent, ElementOnGestureContext, ElementOnKeyboardContext, ElementTree,
};
use crate::core::event::queue::AppEventQueue;
use crate::core::event::AppEvent;
use crate::core::focus::FocusManager;
use crate::core::fonts::FontManager;
use crate::core::gesture::GestureEventComposer;
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::js_command::JsCommandQueue;
use crate::core::keyboard::AppKeyEvent;
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

    pub fn dispatch_handlers(&mut self, event: &AppEvent) {
        let tree = self.element_tree.borrow();
        let mut focus = self.focus_manager.borrow_mut();
        let mut js_eq = self.js_command_queue.borrow_mut();
        let mut cx = HandlerContext {
            element_tree: &tree,
            focus_manager: &mut focus,
            js_command_queue: &mut js_eq,
        };
        for handler in &mut self.handlers {
            handler.handle_event(&mut cx, event);
        }
    }

    pub fn render(&mut self) {
        let (width, height) = self.size;
        let constraints = Constraints {
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        };

        {
            let mut tree = self.element_tree.borrow_mut();
            tree.compute_layout(
                &constraints,
                &mut self.font_manager,
                &mut self.text_layout_cx,
            );
        }

        let focused_node_id = self.focus_manager.borrow().focused();
        let tree = self.element_tree.borrow();
        self.renderer.render(&tree, focused_node_id);
    }

    pub fn local_position(&self, node_id: ElementNodeId, global: tur_shared::Offset) -> tur_shared::Offset {
        let tree = self.element_tree.borrow();
        let mut abs_x = 0.0f64;
        let mut abs_y = 0.0f64;
        let mut current = Some(node_id);
        while let Some(cid) = current {
            if let Some(n) = tree.get(cid) {
                abs_x += n.computed_layout.offset.x;
                abs_y += n.computed_layout.offset.y;
                current = n.parent;
            } else {
                break;
            }
        }
        tur_shared::Offset::new(global.x - abs_x, global.y - abs_y)
    }

    pub fn handle_gesture_event(
        &mut self,
        node_id: ElementNodeId,
        event: &ComposedGestureEvent,
    ) {
        let mut focus = self.focus_manager.borrow_mut();
        let mut js_eq = self.js_command_queue.borrow_mut();
        let mut cx = ElementOnGestureContext::new(
            &mut self.event_queue,
            &mut focus,
            &mut js_eq,
            node_id,
        );

        let mut tree = self.element_tree.borrow_mut();
        let node = match tree.get_mut(node_id) {
            Some(n) => n,
            None => return,
        };
        let element = match node.element.as_mut() {
            Some(e) => e,
            None => return,
        };
        element.on_gesture_event(event, &mut cx);
    }

    pub fn handle_key_event(&mut self, event: &AppKeyEvent) {
        let focused_id = match self.focus_manager.borrow().focused() {
            Some(id) => id,
            None => return,
        };

        let mut js_eq = self.js_command_queue.borrow_mut();
        let mut cx = ElementOnKeyboardContext::new(
            &mut js_eq,
            &mut self.event_queue,
            focused_id,
        );

        let mut tree = self.element_tree.borrow_mut();
        let node = tree.get_mut(focused_id).unwrap();
        let element = node.element.as_mut().unwrap();
        element.on_keyboard_event(&mut cx, event);
    }
}
