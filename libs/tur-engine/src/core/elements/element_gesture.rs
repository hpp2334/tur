use tur_shared::Offset;

use crate::core::element::ElementNodeId;
use crate::core::edgy_event::{EdgyMutation, EventArg, PendingMutationInvocationQueue};
use crate::core::event::queue::AppEventQueue;
use crate::core::event::AppEvent;
use crate::core::focus::FocusManager;

pub enum ComposedGestureEvent {
    PointerDown { local_position: Offset },
    PointerMove { local_position: Offset },
}

pub struct ElementOnGestureContext<'a> {
    event_queue: &'a mut AppEventQueue,
    focus_manager: &'a mut FocusManager,
    mutation_queue: &'a mut PendingMutationInvocationQueue,
    node_id: ElementNodeId,
}

impl<'a> ElementOnGestureContext<'a> {
    pub fn new(
        event_queue: &'a mut AppEventQueue,
        focus_manager: &'a mut FocusManager,
        mutation_queue: &'a mut PendingMutationInvocationQueue,
        node_id: ElementNodeId,
    ) -> Self {
        Self {
            event_queue,
            focus_manager,
            mutation_queue,
            node_id,
        }
    }

    pub fn request_redraw(&mut self) {
        self.event_queue.push(AppEvent::RequestDraw);
    }

    /// Request that the scroll-view node be scrolled to an absolute offset.
    /// Resolved post-dispatch by the `ScrollToHandler` (the tree is mutably
    /// borrowed for the duration of a gesture event, so we defer).
    pub fn request_scroll_to(&mut self, node_id: ElementNodeId, offset: f64) {
        self.event_queue
            .push(AppEvent::ScrollTo { node_id, offset });
    }

    pub fn request_focus(&mut self, id: ElementNodeId) {
        self.focus_manager.set_focus(id);
    }

    pub fn request_own_focus(&mut self) {
        self.focus_manager.set_focus(self.node_id);
    }

    pub fn push_event<E: EventArg>(&mut self, mutation: EdgyMutation<E>, event: E) {
        self.mutation_queue.push(mutation, event);
    }
}

pub trait ElementOnGesture: 'static {
    fn on_gesture_event(
        &mut self,
        cx: &mut ElementOnGestureContext,
        event: &ComposedGestureEvent,
    ) {
        let _ = cx;
        let _ = event;
    }
}
