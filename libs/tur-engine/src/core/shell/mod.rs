//! The interactive layer between an app and the OS — one window's
//! two-way OS boundary.
//!
//! - **Ingress** (OS → engine): [`ShellEvent`] — raw input primitives the
//!   embedder forwards into the engine inside the
//!   [`PlatformEvent::Shell`](crate::core::platform::PlatformEvent::Shell)
//!   envelope.
//! - **Egress** (engine → OS): [`Shell`] — requests the engine pushes to
//!   the window: the resolved [`Cursor`] shape and the text-input (IME)
//!   session state ([`TextInputState`]).
//!
//! A shell targets one specific window, so it is **per-instance and
//! host-thread-only** — a sibling of the renderer (also supplied to
//! `TurAppBuilder` at construction), NOT a runtime capability (capabilities
//! are process-wide and shared across instances). The worker ships deduped
//! egress as `HostMsg::Shell(ShellCommand)`; the host-side `HostBackend`
//! applies it to the embedder-supplied shell inside `apply_msg`.
//!
//! Construction-time (no `set_shell`): the worker's dedup caches start
//! empty, so the very first pump already ships an initial
//! `TextInputState { is_editable: false, .. }` — an embedder that installs
//! its shell after `build()` could miss it (wasm returns before worker
//! readiness). Supplying the shell at construction guarantees it observes
//! every ship from frame 1.

pub mod cursor;
pub mod event;

pub use cursor::Cursor;
pub use event::{ImeEvent, PointerDeviceKind, PointerInput, ShellEvent};

/// Requests the engine pushes to the OS window. Implemented by embedders
/// (wasm's `WasmShell`, Android's `AndroidShell`, test recorders) and
/// installed per-instance via
/// [`TurAppBuilder::shell`](crate::core::runtime::TurAppBuilder::shell);
/// [`NoopShell`] is the default when none is supplied.
///
/// Both methods run on the **host thread** inside `HostBackend::apply_msg`
/// (identical on every driving path — the autonomous
/// `TurAppLooper::run` loop and test-harness pumping alike), so
/// implementations may touch host-thread-only OS APIs (the DOM on wasm,
/// the JNI/Kotlin main looper on Android) directly.
pub trait Shell {
    /// The resolved pointer shape changed (deepest painted `MouseRegion`
    /// claim, deduped — only shipped on change).
    fn set_cursor(&mut self, cursor: Cursor);

    /// The focused element's text-input session state changed: whether an
    /// editable is focused (IME active) and, if so, the logical-space
    /// `(x, y, w, h)` caret rect to anchor composition windows / position a
    /// hidden input. Deduped — only shipped on change.
    fn request_text_input(&mut self, state: TextInputState);
}

/// No-op [`Shell`] default — cursor + text-input requests are silently
/// dropped. Used by headless instances and embedders that pass no shell.
pub struct NoopShell;

impl Shell for NoopShell {
    fn set_cursor(&mut self, _cursor: Cursor) {}
    fn request_text_input(&mut self, _state: TextInputState) {}
}

/// Snapshot of the focused element's text-input session state — whether an
/// editable is focused (IME active) and where its caret is. Pushed to the
/// embedder's [`Shell::request_text_input`] (deduped, on change).
///
/// Note this is *not* general focus state: it reports editable↔non-editable
/// transitions and caret moves only — focus moving between two
/// non-editables never changes it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextInputState {
    pub is_editable: bool,
    /// Logical-space `(x, y, w, h)` of the focused element's caret, or
    /// `None` if no editable is focused.
    pub cursor_rect: Option<(f64, f64, f64, f64)>,
}
