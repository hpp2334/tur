use crate::core::edgy_event::{EdgyMutation, IntoJsArgs, PendingMutationInvocationQueue};
use crate::core::event::queue::AppEventQueue;
use crate::core::event::AppEvent;
use crate::core::keyboard::AppKeyEvent;
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

    pub fn push_event<E: IntoJsArgs>(&mut self, mutation: EdgyMutation<E>, event: E) {
        self.mutation_queue.push(mutation, event);
    }

    pub fn request_paint(&mut self) {
        self.need_paint.set(true);
    }

    /// Request that the embedder write `text` to the system clipboard.
    /// The embedder (e.g. tur-wasm) processes this via a host bridge; in
    /// test harnesses without a clipboard, it's a no-op.
    pub fn push_clipboard_write(&mut self, text: String) {
        self.app_event_queue.push(AppEvent::ClipboardWrite { text });
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
