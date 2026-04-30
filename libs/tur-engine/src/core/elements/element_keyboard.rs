use crate::core::element::ElementNodeId;
use crate::core::js_event::{IntoAnyJsEvent, JsEventQueue};
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

    pub fn push_js_event(&mut self, event: impl IntoAnyJsEvent) {
        self.queue.push(self.node_id, event);
    }

    pub fn request_redraw(&mut self) {
        *self.redraw_requested = true;
    }
}

pub trait ElementOnKeyboard: 'static {
    fn on_keyboard_event(
        &mut self,
        cx: &mut ElementOnKeyboardContext,
        event: &AppKeyEvent,
    ) -> KeyboardResult {
        let _ = cx;
        let _ = event;
        KeyboardResult::NotHandled
    }
}
