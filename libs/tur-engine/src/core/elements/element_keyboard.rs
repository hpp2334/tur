use crate::core::mutation::{MutationHandle, IntoJsArgs, PendingMutationInvocationQueue};
use crate::core::event::queue::AppEventQueue;
use crate::core::keyboard::KeyEvent;
use std::cell::Cell;

pub struct ElementOnKeyboardContext<'a> {
    mutation_queue: &'a mut PendingMutationInvocationQueue,
    app_event_queue: &'a mut AppEventQueue,
    need_paint: &'a Cell<bool>,
}

impl<'a> ElementOnKeyboardContext<'a> {
    pub fn new(
        mutation_queue: &'a mut PendingMutationInvocationQueue,
        app_event_queue: &'a mut AppEventQueue,
        need_paint: &'a Cell<bool>,
    ) -> Self {
        Self {
            mutation_queue,
            app_event_queue,
            need_paint,
        }
    }

    pub fn push_event<E: IntoJsArgs>(&mut self, mutation: MutationHandle<E>, event: E) {
        self.mutation_queue.push(mutation, event);
    }

    pub fn request_paint(&mut self) {
        self.need_paint.set(true);
    }

    /// Borrow the engine-internal event queue so callers can enqueue
    /// domain-specific [`AppEvent::Custom`](crate::core::event::AppEvent::Custom)
    /// payloads (e.g. clipboard write requests) via the matching capability
    /// crate's helper. The engine itself no longer owns clipboard-specific
    /// event-construction helpers.
    pub fn app_event_queue(&mut self) -> &mut AppEventQueue {
        self.app_event_queue
    }
}

pub trait ElementOnKeyboard: 'static {
    fn on_keyboard_event(
        &mut self,
        cx: &mut ElementOnKeyboardContext,
        event: &KeyEvent,
    ) {
        let _ = cx;
        let _ = event;
    }
}
