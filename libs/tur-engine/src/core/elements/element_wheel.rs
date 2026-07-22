use crate::core::edgy::mutation::{MutationHandle, IntoJsArgs, PendingMutationInvocationQueue};
use crate::core::element::ElementNodeId;
use crate::core::app::AppEventQueue;
use crate::core::app::AppEvent;
use std::cell::Cell;

pub struct WheelEvent {
    pub delta_x: f64,
    pub delta_y: f64,
}

pub struct ElementOnWheelContext<'a> {
    event_queue: &'a mut AppEventQueue,
    mutation_queue: &'a mut PendingMutationInvocationQueue,
    need_paint: &'a Cell<bool>,
    node_id: ElementNodeId,
}

impl<'a> ElementOnWheelContext<'a> {
    pub fn new(
        event_queue: &'a mut AppEventQueue,
        mutation_queue: &'a mut PendingMutationInvocationQueue,
        need_paint: &'a Cell<bool>,
        node_id: ElementNodeId,
    ) -> Self {
        Self {
            event_queue,
            mutation_queue,
            need_paint,
            node_id,
        }
    }

    pub fn request_paint(&mut self) {
        self.need_paint.set(true);
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

    pub fn push_event<E: IntoJsArgs>(&mut self, mutation: MutationHandle<E>, event: E) {
        self.mutation_queue.push(mutation, event);
    }
}

pub trait ElementOnWheel: 'static {
    fn on_wheel(&mut self, cx: &mut ElementOnWheelContext, event: &WheelEvent) -> f64;
}
