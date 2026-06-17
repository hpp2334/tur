use crate::core::edgy_event::{EdgyMutation, EventArg, PendingMutationInvocationQueue};
use crate::core::event::queue::AppEventQueue;
use crate::core::event::AppEvent;
use crate::core::event::AppImeEvent;

pub struct ElementOnImeContext<'a> {
    mutation_queue: &'a mut PendingMutationInvocationQueue,
    app_event_queue: &'a mut AppEventQueue,
}

impl<'a> ElementOnImeContext<'a> {
    pub fn new(
        mutation_queue: &'a mut PendingMutationInvocationQueue,
        app_event_queue: &'a mut AppEventQueue,
    ) -> Self {
        Self {
            mutation_queue,
            app_event_queue,
        }
    }

    pub fn push_event<E: EventArg>(&mut self, mutation: EdgyMutation<E>, event: E) {
        self.mutation_queue.push(mutation, event);
    }

    pub fn request_redraw(&mut self) {
        self.app_event_queue.push(AppEvent::RequestDraw);
    }
}

pub trait ElementOnIme: 'static {
    fn on_ime_event(&mut self, cx: &mut ElementOnImeContext, event: &AppImeEvent) {
        let _ = cx;
        let _ = event;
    }
}
