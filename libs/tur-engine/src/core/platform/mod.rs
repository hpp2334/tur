//! Raw input events from the embedder.
//!
//! [`PlatformEvent`] is the **envelope** pushed into the engine via
//! [`TurApp::push_platform_event`](crate::TurApp::push_platform_event):
//! `Shell(ShellEvent)` for raw window-system input (the shell layer's
//! ingress face — see [`crate::core::shell`]) and `Custom(...)` for
//! domain-specific platform events. `KeyEvent` and the
//! `PlatformEventQueue` that buffers events between frames live here.
//!
//! Higher-level gestures (click, drag, scroll-from-touch-drag) are
//! computed *inside* the engine by the gesture arena and the wheel
//! subsystem; they never appear as `PlatformEvent`s.
//!
//! Cursor **output** (the resolved pointer shape the engine pushes back
//! to the window) lives in [`crate::core::shell::cursor`] — it is an
//! egress seam, not an input event.

pub mod event;
pub mod key_event;
pub mod queue;

pub use event::{CustomPlatformEvent, PlatformEvent};
// Re-exported for convenience: the raw input payload types live in the
// shell layer (`core::shell::event`), but every embedder that pushes
// events imports them through `core::platform`.
pub use crate::core::shell::event::{ImeEvent, PointerDeviceKind, PointerInput, ShellEvent};
pub use key_event::{KeyEvent, KeyEventType, KeydownEvent, KeyupEvent, Modifiers};
pub use queue::PlatformEventQueue;
