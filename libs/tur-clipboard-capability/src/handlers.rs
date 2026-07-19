//! Engine-internal clipboard subsystems — the Cmd+C/Cmd+X write path and
//! the paste-forwarding path.
//!
//! ## Write (Cmd+C / Cmd+X)
//!
//! tur-text's `EditableTextElement` calls [`push_write`](crate::events::push_write)
//! on Cmd+C / Cmd+X, which enqueues a [`ClipboardWriteEvent`] on the
//! engine-internal bus. [`ClipboardWriteSubsystem`] consumes it and drives
//! the [`Clipboard`] backend's `write_text` via the engine's async executor.
//!
//! ## Paste (Cmd+V)
//!
//! The embedder wraps the platform paste as a [`ClipboardPlatformPasteEvent`]
//! via [`platform_paste`](crate::events::platform_paste) and pushes it on
//! the platform queue. [`ClipboardPlatformSubsystem`] consumes it and
//! re-emits a [`ClipboardPasteEvent`] on the engine-internal bus; tur-text's
//! `ClipboardPasteSubsystem` then inserts the text into the focused
//! `EditableTextElement`.

use tur_engine::core::event::{AppEvent, PlatformEvent};
use tur_engine::core::subsystem::{Subsystem, SubsystemFlushContext};

use crate::events::{ClipboardPlatformPasteEvent, ClipboardWriteEvent, push_paste};
use crate::Clipboard;

/// Forwards [`ClipboardPlatformPasteEvent`] (embedder → engine) as a
/// [`ClipboardPasteEvent`] on the engine-internal bus. tur-text's
/// `ClipboardPasteSubsystem` then consumes the AppEvent and inserts the text
/// into the focused editable.
///
/// This split keeps the engine's platform boundary thin (no knowledge of
/// specific element types) while letting tur-text own the paste logic next
/// to `EditableTextElement`. Keeping the caret on screen after a paste is
/// handled by tur-text's `CaretVisibilitySubsystem`, which (like the paste
/// subsystem) subscribes to [`ClipboardPasteEvent`] and is registered after
/// `ClipboardPasteSubsystem` in registration order.
pub struct ClipboardPlatformSubsystem;

impl Subsystem for ClipboardPlatformSubsystem {
    fn handle_platform_event(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        event: &PlatformEvent,
    ) {
        let Some(ev) = event.as_custom::<ClipboardPlatformPasteEvent>() else {
            return;
        };
        push_paste(cx.app_event_queue, ev.text.clone());
    }
}

/// Handles [`ClipboardWriteEvent`] (produced by EditableText on Cmd+C /
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
    fn handle_app_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &AppEvent) {
        let Some(ev) = event.as_custom::<ClipboardWriteEvent>() else {
            return;
        };
        let text = ev.text.clone();
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
