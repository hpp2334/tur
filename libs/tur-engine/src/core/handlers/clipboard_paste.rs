use crate::core::event::{AppEvent, PlatformEvent};
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

/// Forwards [`PlatformEvent::ClipboardPaste`] (embedder → engine) as
/// [`AppEvent::ClipboardPaste`] on the engine-internal event bus. tur-text's
/// `ClipboardPasteSubsystem` then consumes the AppEvent and inserts the text
/// into the focused editable.
///
/// This split keeps the engine's platform boundary thin (no knowledge of
/// specific element types) while letting tur-text own the paste logic next
/// to `EditableTextElement`. Keeping the caret on screen after a paste is
/// handled by tur-text's `CaretVisibilitySubsystem`, which (like the paste
/// subsystem) subscribes to `AppEvent::ClipboardPaste` and is registered
/// after `ClipboardPasteSubsystem` in registration order.
pub struct ClipboardPlatformSubsystem;

impl Subsystem for ClipboardPlatformSubsystem {
    fn handle_platform_event(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        event: &PlatformEvent,
    ) {
        let PlatformEvent::ClipboardPaste { text } = event else {
            return;
        };
        cx.app_event_queue
            .push(AppEvent::ClipboardPaste { text: text.clone() });
    }
}
