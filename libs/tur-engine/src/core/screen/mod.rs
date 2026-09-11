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

/// The viewport a frame was laid out for — the worker-owned half of the
/// render commit.
///
/// Stamped on every `HostMsg::RenderCommands` batch (read from the
/// worker's [`Screen`] at record time); the host applies it to its
/// renderer immediately before playing the batch back — the single
/// **render commit point**. Synchronizing geometry only there makes the
/// backing-store swap and the frame's content land in one operation, so
/// the renderer's currently-presented frame is never destroyed by a resize
/// that arrives before its replacement frame does (the resize white
/// flash).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenViewport {
    /// Logical width in CSS pixels.
    pub logical_width: u32,
    /// Logical height in CSS pixels.
    pub logical_height: u32,
    /// Device pixel ratio at record time.
    pub dpr: f64,
}

/// Engine screen state — pure data: the canvas's logical size + DPR.
/// Updated by [`ResizeSubsystem`] on shell `Resize` events (which also
/// publishes the new size into `viewportSize$`).
pub struct Screen {
    /// Current canvas logical size, in CSS pixels.
    pub logical_size: (f64, f64),
    /// Current device pixel ratio. Shipped to main with each
    /// `HostMsg::RenderCommands` (inside the batch's
    /// [`ScreenViewport`](super::ScreenViewport)) so the host-side renderer
    /// syncs at the render commit point + applies the dpr root transform.
    pub dpr: f64,
}

impl Screen {
    /// Snapshot the current viewport ([`ScreenViewport`]) — what a batch
    /// being shipped to the host was laid out for.
    pub fn viewport(&self) -> ScreenViewport {
        ScreenViewport {
            logical_width: self.logical_size.0 as u32,
            logical_height: self.logical_size.1 as u32,
            dpr: self.dpr,
        }
    }
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
