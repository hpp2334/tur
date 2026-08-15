//! Clipboard event payloads travelling on the engine's two event buses.
//!
//! The engine's [`PlatformEvent`](crate::core::platform::PlatformEvent) /
//! [`AppEvent`](crate::core::app::AppEvent) enums carry only generic
//! primitives plus a single `Custom` escape hatch. This module defines the
//! clipboard-specific payload types and the helpers that wrap them into the
//! engine's queues — so neither the engine nor the embedder needs to
//! pattern-match a clipboard variant.
//!
//! These types live inside the clipboard plugin (not `core/`) because they
//! are owned entirely by this plugin's data flow:
//!
//! - **Consumer**: `builtin_plugins/text` (paste → `EditableTextElement`,
//!   copy/cut → write request) reads `ClipboardPasteEvent` /
//!   `ClipboardWriteEvent`. Cross-plugin import via
//!   `pub(in crate::builtin_plugins)`.
//! - **Producer**: this plugin's own [`handlers`] (`ClipboardShellSubsystem`
//!   forwards embedder paste; `ClipboardWriteSubsystem` drains writes).
//! - **Embedder**: wraps platform paste via [`shell_paste`] (the one
//!   `pub` helper, re-exported at the engine crate root).
//!
//! ## Three roles
//!
//! | Type | Channel | Producer | Consumer |
//! |---|---|---|---|
//! | [`ClipboardShellPasteEvent`] | `ShellEventPayload::Custom` | embedder (tur-wasm / tests) | `ClipboardShellSubsystem` in this plugin |
//! | [`ClipboardPasteEvent`] | `AppEvent::Custom` | `ClipboardShellSubsystem` (forwarder) | text plugin's `ClipboardPasteSubsystem` + `CaretVisibilitySubsystem` |
//! | [`ClipboardWriteEvent`] | `AppEvent::Custom` | text plugin's `EditableTextElement` (Cmd+C/X) | `ClipboardWriteSubsystem` in this plugin |
//!
//! `ClipboardShellPasteEvent` and `ClipboardPasteEvent` have the same
//! shape but live on different channels with different semantics — the
//! shell-injected paste becomes an engine-internal paste only after the
//! forwarder acknowledges it. Keeping them as distinct types prevents an
//! embedder from accidentally pushing an `AppEvent` (or vice versa).

use crate::core::app::{AppEvent, AppEventQueue, CustomAppEvent};
use crate::core::element::ViewRootId;
use crate::core::platform::{CustomShellEventPayload, PlatformEvent};

/// Embedder → engine: a paste occurred (the user pressed Cmd+V / Ctrl+V, the
/// embedder captured the paste event on its hidden input, and is forwarding
/// the clipboard text). Travels inside `ShellEventPayload::Custom`. Consumed by
/// [`ClipboardShellSubsystem`](crate::builtin_plugins::clipboard::handlers::ClipboardShellSubsystem),
/// which re-emits it on the engine-internal bus as a [`ClipboardPasteEvent`].
#[derive(Debug)]
pub struct ClipboardShellPasteEvent {
    pub text: String,
}

impl CustomShellEventPayload for ClipboardShellPasteEvent {
    fn name(&self) -> &'static str {
        "clipboard.shell_paste"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Engine-internal paste request — the paste text to insert into the focused
/// editable. Produced by `ClipboardShellSubsystem` (which forwards the
/// embedder's [`ClipboardShellPasteEvent`]) and consumed by text plugin's
/// `ClipboardPasteSubsystem`, which does the actual insertion. Lives on the
/// AppEvent bus so the paste logic stays in the text plugin next to
/// `EditableTextElement` instead of requiring a per-element trait in the
/// engine.
#[derive(Debug)]
pub struct ClipboardPasteEvent {
    pub text: String,
}

impl CustomAppEvent for ClipboardPasteEvent {
    fn name(&self) -> &'static str {
        "clipboard.paste"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Engine → embedder: write `text` to the system clipboard (copy / cut).
/// Produced by text plugin's `EditableTextElement` on Cmd+C / Cmd+X and
/// consumed by `ClipboardWriteSubsystem` (in this plugin), which drives the
/// injected `Clipboard` capability.
#[derive(Debug)]
pub struct ClipboardWriteEvent {
    pub text: String,
}

impl CustomAppEvent for ClipboardWriteEvent {
    fn name(&self) -> &'static str {
        "clipboard.write"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Queue helpers — wrap the payload types so producers don't pattern-match
// `Custom` themselves.
// ---------------------------------------------------------------------------

/// Wrap a paste payload as a shell event targeting `view_root_id`, ready to
/// be pushed onto the platform queue. Used by embedders (tur-wasm,
/// integration tests). Re-exported at the engine crate root as
/// `tur_engine::shell_paste`.
pub fn shell_paste(view_root_id: ViewRootId, text: String) -> PlatformEvent {
    PlatformEvent::shell(
        view_root_id,
        crate::core::platform::ShellEventPayload::Custom(Box::new(ClipboardShellPasteEvent {
            text,
        })),
    )
}

/// Push an engine-internal paste request onto `queue`. Used by
/// `ClipboardShellSubsystem` to forward the embedder's paste into the
/// engine-internal bus.
pub(in crate::builtin_plugins) fn push_paste(queue: &mut AppEventQueue, text: String) {
    queue.push(AppEvent::custom(ClipboardPasteEvent { text }));
}

/// Push a clipboard-write request onto `queue`. Used by text plugin's
/// `EditableTextElement` on Cmd+C / Cmd+X.
pub(in crate::builtin_plugins) fn push_write(queue: &mut AppEventQueue, text: String) {
    queue.push(AppEvent::custom(ClipboardWriteEvent { text }));
}
