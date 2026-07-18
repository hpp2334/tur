//! Engine-internal clipboard write subsystem — the Cmd+C/Cmd+X path.
//!
//! Paste (Cmd+V) is not handled here. The embedder pushes
//! `PlatformEvent::ClipboardPaste`, which the engine's
//! `ClipboardPlatformSubsystem` forwards as `AppEvent::ClipboardPaste`;
//! tur-text's `ClipboardPasteSubsystem` then consumes the AppEvent and
//! inserts the text into the focused `EditableTextElement`.

use tur_engine::core::event::AppEvent;
use tur_engine::core::subsystem::{Subsystem, SubsystemFlushContext};

use crate::Clipboard;

/// Handles `AppEvent::ClipboardWrite` (produced by EditableText on Cmd+C /
/// Cmd+X) by spawning `clipboard.write_text(text)` on the engine's async
/// executor. Looks up the [`Clipboard`] capability at dispatch time via
/// `cx.capabilities.of::<Clipboard>()`; silently drops the write (with a
/// warning) if no backend is registered — though `TurClipboardPlugin`'s
/// `requires` declaration should prevent that at `build()` time.
///
/// The async executor is sourced from [`SubsystemFlushContext`] at dispatch
/// time, so this subsystem holds no executor state of its own.
pub struct ClipboardWriteSubsystem;

impl Subsystem for ClipboardWriteSubsystem {
    fn handle_app_event(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        event: &AppEvent,
    ) {
        let AppEvent::ClipboardWrite { text } = event else {
            return;
        };
        let text = text.clone();
        let Some(clipboard_cap) = cx.capabilities.of::<Clipboard>() else {
            tracing::warn!("ClipboardWrite dropped: no Clipboard capability registered");
            return;
        };
        let backend = clipboard_cap.backend().clone();
        cx.async_executor.spawn_detached(async move {
            backend.write_text(text).await;
        });
    }
}
