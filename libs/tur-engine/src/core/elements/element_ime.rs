use crate::core::edgy::mutation::{IntoJsArgs, MutationHandle, PendingMutationInvocationQueue};
use crate::core::platform::ImeEvent;
use std::cell::Cell;

pub struct ElementOnImeContext<'a> {
    mutation_queue: &'a mut PendingMutationInvocationQueue,
    need_paint: &'a Cell<bool>,
}

impl<'a> ElementOnImeContext<'a> {
    pub fn new(
        mutation_queue: &'a mut PendingMutationInvocationQueue,
        need_paint: &'a Cell<bool>,
    ) -> Self {
        Self {
            mutation_queue,
            need_paint,
        }
    }

    pub fn push_event<E: IntoJsArgs>(&mut self, mutation: MutationHandle<E>, event: E) {
        self.mutation_queue.push(mutation, event);
    }

    pub fn request_paint(&mut self) {
        self.need_paint.set(true);
    }
}

pub trait ElementOnIme: 'static {
    fn on_ime_event(&mut self, cx: &mut ElementOnImeContext, event: &ImeEvent) {
        let _ = cx;
        let _ = event;
    }
}
