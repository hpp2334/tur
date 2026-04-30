use crate::core::element::ElementNodeId;
use crate::core::event::queue::AppEventQueue;
use crate::core::event::AppEvent;
use crate::core::js_event::{IntoAnyJsEvent, JsEventQueue};
use crate::core::keyboard::AppKeyEvent;

pub struct ElementOnKeyboardContext<'a> {
    js_event_queue: &'a mut JsEventQueue,
    app_event_queue: &'a mut AppEventQueue,
    node_id: ElementNodeId,
}

impl<'a> ElementOnKeyboardContext<'a> {
    pub fn new(
        js_event_queue: &'a mut JsEventQueue,
        app_event_queue: &'a mut AppEventQueue,
        node_id: ElementNodeId,
    ) -> Self {
        Self {
            js_event_queue,
            app_event_queue,
            node_id,
        }
    }

    pub fn push_js_event(&mut self, event: impl IntoAnyJsEvent) {
        self.js_event_queue.push(self.node_id, event);
    }

    pub fn request_redraw(&mut self) {
        self.app_event_queue.push(AppEvent::RequestDraw);
    }
}

pub trait ElementOnKeyboard: 'static {
    fn on_keyboard_event(
        &mut self,
        cx: &mut ElementOnKeyboardContext,
        event: &AppKeyEvent,
    ) {
        let _ = cx;
        let _ = event;
    }
}
