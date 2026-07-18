use crate::core::event::{AppEvent, PlatformEvent};
use crate::core::handler::{AppHandler, HandlerContext};

/// Forwards [`PlatformEvent::ClipboardPaste`] (embedder → engine) as
/// [`AppEvent::ClipboardPaste`] on the engine-internal event bus. tur-text's
/// `ClipboardPasteHandler` then consumes the AppEvent and inserts the text
/// into the focused editable.
///
/// This split keeps the engine's platform boundary thin (no knowledge of
/// specific element types) while letting tur-text own the paste logic next
/// to `EditableTextElement`. Keeping the caret on screen after a paste is
/// handled by tur-text's `EnsureCaretVisibleHandler`, which (like the paste
/// handler) subscribes to `AppEvent::ClipboardPaste` and is registered after
/// `ClipboardPasteHandler` in registration order.
pub struct ClipboardPasteAppHandler;

impl AppHandler for ClipboardPasteAppHandler {
    fn handle_platform_event(&mut self, cx: &mut HandlerContext, event: &PlatformEvent) {
        let PlatformEvent::ClipboardPaste { text } = event else {
            return;
        };
        cx.app_event_queue
            .push(AppEvent::ClipboardPaste { text: text.clone() });
    }
}
