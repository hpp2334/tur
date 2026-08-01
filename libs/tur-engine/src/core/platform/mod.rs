//! Platform capability traits + raw input events from the embedder.
//!
//! Two concerns live here:
//!
//! 1. **Cursor output** (`cursor` submodule) — `Cursor` enum +
//!    `CursorBackend` trait + `CursorCap` capability newtype. Registered
//!    via `TurRuntimeBuilder::capability(CursorCap::new(...))`; the engine
//!    builder installs the backend on the [`Shell`](crate::core::shell::Shell)
//!    at build time.
//!
//! 2. **Raw input events** (`event` + `queue` submodules) — `PlatformEvent`,
//!    `PointerInput`, `ImeEvent`, and the `PlatformEventQueue` that buffers
//!    them between frames. These describe what the host window system /
//!    browser DOM just told the engine. Higher-level gestures (click, drag,
//!    scroll-from-touch-drag) are computed *inside* the engine by the gesture
//!    arena and the wheel subsystem; they never appear as `PlatformEvent`s.

pub mod cursor;
pub mod event;
pub mod key_event;
pub mod queue;

pub use cursor::{Cursor, CursorBackend, CursorCap, NoopCursor};
pub use event::{CustomPlatformEvent, ImeEvent, PlatformEvent, PointerDeviceKind, PointerInput};
pub use key_event::{KeyEvent, KeyEventType, KeydownEvent, KeyupEvent, Modifiers};
pub use queue::PlatformEventQueue;
