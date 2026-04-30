use std::any::Any;
use std::rc::Rc;

use crate::core::element::ElementNodeId;
use crate::core::js_event::JsEventQueue;
use crate::core::keyboard::AppKeyEvent;

pub enum KeyboardResult {
    NotHandled,
    Handled,
    NeedsDraw,
}

pub struct ElementOnKeyboardContext<'a> {
    queue: &'a mut JsEventQueue,
    node_id: ElementNodeId,
    redraw_requested: &'a mut bool,
}

impl<'a> ElementOnKeyboardContext<'a> {
    pub fn new(
        queue: &'a mut JsEventQueue,
        node_id: ElementNodeId,
        redraw_requested: &'a mut bool,
    ) -> Self {
        Self {
            queue,
            node_id,
            redraw_requested,
        }
    }

    pub fn push_js_event(&mut self, event: impl Any + 'static) {
        self.queue.push(self.node_id, Rc::new(event));
    }

    pub fn request_redraw(&mut self) {
        *self.redraw_requested = true;
    }
}

pub trait ElementOnKeyboard: 'static {
    fn on_keyboard_event(
        &mut self,
        event: &AppKeyEvent,
        cx: &mut ElementOnKeyboardContext,
    ) -> KeyboardResult {
        let _ = event;
        let _ = cx;
        KeyboardResult::NotHandled
    }
}
