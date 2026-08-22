//! Screen domain — the canvas's logical size + the resize subsystem that
//! drives it.
//!
//! [`Screen`] is **pure data**: the current logical size (CSS pixels) and
//! the device pixel ratio. The `viewportSize$` atom — its backing source,
//! the instance-store write rail it publishes through, and the dedup guard
//! — is owned by [`ResizeSubsystem`] (`resize.rs`), minted and registered
//! by `TurStdPlugin` (the canonical plugin-facing engine-atom recipe), and
//! driven by the shell `Resize` event (updates this state, publishes the
//! atom, and re-marks the tree root dirty).
//!
//! `TurAppContext` owns a [`Screen`] inline; `SubsystemFlushContext.screen`
//! is a `&mut` borrow into it, so the resize handler drives both the size
//! mutation and the atom publish directly (event-driven, not polled each
//! frame).

pub mod resize;

pub use resize::ResizeSubsystem;

pub(crate) use resize::viewport_size_value;

/// Engine screen state — pure data: the canvas's logical size + DPR.
/// Updated by [`ResizeSubsystem`] on shell `Resize` events (which also
/// publishes the new size into `viewportSize$`).
pub struct Screen {
    /// Current canvas logical size, in CSS pixels.
    pub logical_size: (f64, f64),
    /// Current device pixel ratio. Shipped to main with each
    /// `HostMsg::RenderCommands` so the host-side renderer can call
    /// `resize()` + apply the dpr root transform.
    pub dpr: f64,
}

impl Screen {
    /// Create with the default initial logical size (400×600) — matches the
    /// historical `TurAppContext::new` default before this type existed.
    /// The engine builder overwrites `logical_size` with the real viewport
    /// before anything runs.
    pub fn new() -> Self {
        Self {
            logical_size: (400.0, 600.0),
            dpr: 1.0,
        }
    }
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}
