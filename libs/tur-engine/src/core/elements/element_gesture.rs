use tur_shared::Offset;

use crate::core::element::ElementNodeId;
use crate::core::event::queue::AppEventQueue;
use crate::core::event::AppEvent;
use crate::core::focus::FocusManager;
use crate::core::js_command::{IntoAnyJsCommand, JsCommandQueue};

pub enum ComposedGestureEvent {
    PointerDown { local_position: Offset },
    PointerMove { local_position: Offset },
    Wheel { delta_x: f64, delta_y: f64 },
}

pub struct ElementOnGestureContext<'a> {
    event_queue: &'a mut AppEventQueue,
    focus_manager: &'a mut FocusManager,
    js_command_queue: &'a mut JsCommandQueue,
    node_id: ElementNodeId,
}

impl<'a> ElementOnGestureContext<'a> {
    pub fn new(
        event_queue: &'a mut AppEventQueue,
        focus_manager: &'a mut FocusManager,
        js_command_queue: &'a mut JsCommandQueue,
        node_id: ElementNodeId,
    ) -> Self {
        Self {
            event_queue,
            focus_manager,
            js_command_queue,
            node_id,
        }
    }

    pub fn request_redraw(&mut self) {
        self.event_queue.push(AppEvent::RequestDraw);
    }

    pub fn request_focus(&mut self, id: ElementNodeId) {
        self.focus_manager.set_focus(id, self.js_command_queue);
    }

    pub fn request_own_focus(&mut self) {
        self.focus_manager.set_focus(self.node_id, self.js_command_queue);
    }

    pub fn push_js_command(&mut self, command: impl IntoAnyJsCommand) {
        self.js_command_queue.push(self.node_id, command);
    }
}

pub trait ElementOnGesture: 'static {
    fn on_gesture_event(
        &mut self,
        cx: &mut ElementOnGestureContext,
        event: &ComposedGestureEvent,
    ) -> bool {
        let _ = cx;
        let _ = event;
        false
    }
}
