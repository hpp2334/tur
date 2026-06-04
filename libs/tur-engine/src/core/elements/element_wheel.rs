use crate::core::element::ElementNodeId;
use crate::core::event::queue::AppEventQueue;
use crate::core::event::AppEvent;
use crate::core::js_command::{IntoAnyJsCommand, JsCommandQueue};

pub struct WheelEvent {
    pub delta_x: f64,
    pub delta_y: f64,
}

pub struct ElementOnWheelContext<'a> {
    event_queue: &'a mut AppEventQueue,
    js_command_queue: &'a mut JsCommandQueue,
    node_id: ElementNodeId,
}

impl<'a> ElementOnWheelContext<'a> {
    pub fn new(
        event_queue: &'a mut AppEventQueue,
        js_command_queue: &'a mut JsCommandQueue,
        node_id: ElementNodeId,
    ) -> Self {
        Self {
            event_queue,
            js_command_queue,
            node_id,
        }
    }

    pub fn request_redraw(&mut self) {
        self.event_queue.push(AppEvent::RequestDraw);
    }

    pub fn node_id(&self) -> ElementNodeId {
        self.node_id
    }

    pub fn push_overscroll(&mut self, delta: f64) {
        self.event_queue.push(AppEvent::ScrollOverscroll {
            source_id: self.node_id,
            delta,
        });
    }

    pub fn push_js_command(&mut self, command: impl IntoAnyJsCommand) {
        self.js_command_queue.push(self.node_id, command);
    }
}

pub trait ElementOnWheel: 'static {
    fn on_wheel(&mut self, cx: &mut ElementOnWheelContext, event: &WheelEvent) -> f64;
}
