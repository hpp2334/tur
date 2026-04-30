use tur_shared::Offset;

use crate::core::element::ElementNodeId;

pub enum GestureResult {
    NotHandled,
    Handled,
    NeedsDraw,
}

pub enum ComposedGestureEvent {
    PointerDown { local_position: Offset },
    PointerMove { local_position: Offset },
}

pub struct ElementOnGestureContext<'a> {
    redraw_requested: &'a mut bool,
    focus_request: &'a mut Option<ElementNodeId>,
    node_id: ElementNodeId,
}

impl<'a> ElementOnGestureContext<'a> {
    pub fn new(
        redraw_requested: &'a mut bool,
        focus_request: &'a mut Option<ElementNodeId>,
        node_id: ElementNodeId,
    ) -> Self {
        Self {
            redraw_requested,
            focus_request,
            node_id,
        }
    }

    pub fn request_redraw(&mut self) {
        *self.redraw_requested = true;
    }

    pub fn request_focus(&mut self, id: ElementNodeId) {
        *self.focus_request = Some(id);
    }

    pub fn request_own_focus(&mut self) {
        *self.focus_request = Some(self.node_id);
    }
}

pub trait ElementOnGesture: 'static {
    fn on_gesture_event(
        &mut self,
        event: &ComposedGestureEvent,
        cx: &mut ElementOnGestureContext,
    ) -> GestureResult {
        let _ = event;
        let _ = cx;
        GestureResult::NotHandled
    }
}
