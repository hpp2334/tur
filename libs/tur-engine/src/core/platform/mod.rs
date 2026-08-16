//! Raw input events from the embedder.
//!
//! `PlatformEvent`, `PointerInput`, `ImeEvent`, `KeyEvent`, and the
//! `PlatformEventQueue` that buffers them between frames. These describe
//! what the host window system / browser DOM just told the engine.
//! Higher-level gestures (click, drag, scroll-from-touch-drag) are
//! computed *inside* the engine by the gesture arena and the wheel
//! subsystem; they never appear as `PlatformEvent`s.
//!
//! Cursor **output** (the resolved pointer shape the engine pushes back
//! to the window) lives in [`crate::core::cursor`] — it is an egress
//! seam, not an input event.

pub mod event;
pub mod key_event;
pub mod queue;

pub use event::{CustomPlatformEvent, ImeEvent, PlatformEvent, PointerDeviceKind, PointerInput};
pub use key_event::{KeyEvent, KeyEventType, KeydownEvent, KeyupEvent, Modifiers};
pub use queue::PlatformEventQueue;
