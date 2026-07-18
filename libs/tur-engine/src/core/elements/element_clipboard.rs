use std::cell::Cell;

use crate::core::mutation::{IntoJsArgs, MutationHandle, PendingMutationInvocationQueue};

pub struct ElementOnClipboardContext<'a> {
    mutation_queue: &'a mut PendingMutationInvocationQueue,
    need_paint: &'a Cell<bool>,
}

impl<'a> ElementOnClipboardContext<'a> {
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

/// Element-level clipboard event handling. Currently only covers paste
/// (`PlatformEvent::ClipboardPaste`), dispatched to the focused element by
/// the engine's `ClipboardPasteAppHandler`. Mirrors `ElementOnIme` /
/// `ElementOnKeyboard` — the element reacts in-place and may push mutations
/// (e.g. firing `onInput`) or request a paint.
pub trait ElementOnClipboard: 'static {
    /// Called when the host receives a paste event (e.g. Cmd+V captured by
    /// the embedder on a hidden input) while this element is focused.
    /// `text` is the clipboard contents; default impl is a no-op so
    /// non-editable elements ignore paste.
    fn on_clipboard_paste(&mut self, cx: &mut ElementOnClipboardContext, text: &str) {
        let _ = (cx, text);
    }
}
