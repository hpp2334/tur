//! Engine-internal clipboard subsystems — the Cmd+C/Cmd+X write path and
//! the paste-forwarding path.
//!
//! ## Write (Cmd+C / Cmd+X)
//!
//! tur-text's `EditableTextElement` calls
//! [`push_write`](super::event::push_write) on Cmd+C / Cmd+X, which enqueues
//! a [`ClipboardWriteEvent`] on the engine-internal bus.
//! [`ClipboardWriteSubsystem`] consumes it and drives the
//! [`Clipboard`] backend's `write_text` via the engine's async executor.
//!
//! ## Paste (Cmd+V)
//!
//! The embedder wraps the platform paste as a
//! [`ClipboardPlatformPasteEvent`] via
//! [`platform_paste`](super::event::platform_paste) and pushes it on the
//! platform queue. [`ClipboardPlatformSubsystem`] consumes it and re-emits a
//! [`ClipboardPasteEvent`] on the engine-internal bus; tur-text's
//! `ClipboardPasteSubsystem` then inserts the text into the focused
//! `EditableTextElement`.

use std::pin::Pin;

use crate::core::app::AppEvent;
use crate::core::platform::PlatformEvent;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

use super::capability::Clipboard;
use super::event::{ClipboardPlatformPasteEvent, ClipboardWriteEvent, push_paste};
/// Forwards [`ClipboardPlatformPasteEvent`] (embedder → engine) as a
/// [`ClipboardPasteEvent`](super::event::ClipboardPasteEvent) on the
/// engine-internal bus. tur-text's `ClipboardPasteSubsystem` then consumes
/// the AppEvent and inserts the text into the focused editable.
///
/// This split keeps the engine's platform boundary thin (no knowledge of
/// specific element types) while letting tur-text own the paste logic next
/// to `EditableTextElement`. Keeping the caret on screen after a paste is
/// handled by tur-text's `CaretVisibilitySubsystem`, which (like the paste
/// subsystem) subscribes to `ClipboardPasteEvent` and is registered after
/// `ClipboardPasteSubsystem` in registration order.
pub(in crate::builtin_plugins) struct ClipboardPlatformSubsystem;

impl Subsystem for ClipboardPlatformSubsystem {
    fn handle_platform_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &PlatformEvent) {
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
/// warning) if no backend is registered — though [`TurClipboardPlugin`](super::TurClipboardPlugin)'s
/// `requires` declaration should prevent that at `build()` time.
///
/// The async executor is sourced from [`SubsystemFlushContext`] at dispatch
/// time, so this subsystem holds no executor state of its own.
pub(in crate::builtin_plugins) struct ClipboardWriteSubsystem;

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
        let fut: Pin<Box<dyn std::future::Future<Output = ()> + 'static>> = Box::pin(async move {
            backend.write_text(text).await;
        });
        let _ = cx.worker_ctx.spawn_local(fut);
    }
}
