//! The interactive layer between an app and the OS — one window's
//! two-way OS boundary.
//!
//! - **Ingress** (OS → engine): [`ShellEvent`] — raw input primitives the
//!   embedder forwards into the engine inside the
//!   [`PlatformEvent::Shell`](crate::core::platform::PlatformEvent::Shell)
//!   envelope.
//! - **Egress** (engine → OS): [`Shell`] — requests the engine pushes to
//!   the window: the resolved [`Cursor`] shape, the text-input (IME)
//!   session state ([`TextInputState`]), and the frame clock
//!   ([`VsyncSource`](crate::core::scheduler::VsyncSource), handed over
//!   once at construction).
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
//! every ship from frame 1. The frame clock follows the same pattern: the
//! engine takes it via [`Shell::take_vsync`] exactly once, at
//! construction, so there is nothing left to configure after `build()`.

pub mod cursor;
pub mod event;

pub use cursor::Cursor;
pub use event::{ImeEvent, PointerDeviceKind, PointerInput, ShellEvent};

use crate::core::scheduler::VsyncSource;
use std::rc::Rc;

/// Requests the engine pushes to the OS window. Implemented by embedders
/// (wasm's `WasmShell`, Android's `AndroidShell`, test recorders) and
/// installed per-instance via
/// [`TurAppBuilder::shell`](crate::core::runtime::TurAppBuilder::shell);
/// [`NoopShell`] is the default when none is supplied.
///
/// The methods run on the **host thread** — `set_cursor` /
/// `request_text_input` inside `HostBackend::apply_msg` (identical on
/// every driving path — the autonomous `TurAppLooper::run` loop and
/// test-harness pumping alike) — so implementations may touch
/// host-thread-only OS APIs (the DOM on wasm, the JNI/Kotlin main looper
/// on Android) directly.
pub trait Shell {
    /// The resolved pointer shape changed (deepest painted `MouseRegion`
    /// claim, deduped — only shipped on change).
    fn set_cursor(&mut self, cursor: Cursor);

    /// The focused element's text-input session state changed: whether an
    /// editable is focused (IME active) and, if so, the logical-space
    /// `(x, y, w, h)` caret rect to anchor composition windows / position a
    /// hidden input. Deduped — only shipped on change.
    fn request_text_input(&mut self, state: TextInputState);

    /// Hand over this window's frame clock. The engine takes it exactly
    /// once, at construction (`spawn_instance`), and `None` afterwards —
    /// the slot is consumed. Implementations hold
    /// `vsync: Option<Rc<dyn VsyncSource>>` and return `self.vsync.take()`.
    ///
    /// `build()` fails fast if a shell hands back `None` on the first
    /// take: the autonomous loop needs a cadence (embedders that truly
    /// don't care supply [`NoopVsyncSource`](crate::core::scheduler::NoopVsyncSource)
    /// explicitly).
    fn take_vsync(&mut self) -> Option<Rc<dyn VsyncSource>>;
}

/// No-op [`Shell`] default — cursor + text-input requests are silently
/// dropped, and the frame clock is a
/// [`NoopVsyncSource`](crate::core::scheduler::NoopVsyncSource) (never
/// fires; the loop then progresses on worker messages only). Used by
/// headless instances and embedders that pass no shell.
pub struct NoopShell {
    vsync: Option<Rc<dyn VsyncSource>>,
}

impl NoopShell {
    pub fn new() -> Self {
        Self {
            vsync: Some(Rc::new(crate::core::scheduler::NoopVsyncSource)),
        }
    }
}

impl Default for NoopShell {
    fn default() -> Self {
        Self::new()
    }
}

impl Shell for NoopShell {
    fn set_cursor(&mut self, _cursor: Cursor) {}
    fn request_text_input(&mut self, _state: TextInputState) {}
    fn take_vsync(&mut self) -> Option<Rc<dyn VsyncSource>> {
        self.vsync.take()
    }
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
