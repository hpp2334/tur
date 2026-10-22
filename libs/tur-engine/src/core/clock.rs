//! The engine's time source.
//!
//! One [`Clock`] per [`TurRuntime`](crate::TurRuntime) — a `Send + Sync`
//! `Arc` shared by the host backend (frame-timing probes), the frame
//! environment, subsystems (animation ticking), and the rut rows' virtual
//! clock sync (`now_ms`). The former boa `context::time::Clock` dependency
//! collapsed into this engine-owned trait when the JS rail was deleted.
//!
//! Embedders supply their own implementation (or use [`StdClock`]);
//! tests use [`FixedClock`] / a mutex wrapper for determinism.

use std::time::{SystemTime, UNIX_EPOCH};

/// The engine's time source: milliseconds since the Unix epoch.
pub trait Clock: Send + Sync {
    /// Milliseconds since the Unix epoch. Monotonic-enough for animation
    /// ticking + virtual-clock sync; frame *deltas* should use the vsync
    /// cadence, not this.
    fn now_millis(&self) -> f64;
}

/// The system wall clock (the default time source).
#[derive(Debug, Default, Clone, Copy)]
pub struct StdClock;

impl Clock for StdClock {
    fn now_millis(&self) -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }
}

/// A clock frozen at a fixed instant (tests, headless builds).
#[derive(Debug, Clone, Copy)]
pub struct FixedClock {
    millis: f64,
}

impl FixedClock {
    pub fn from_millis(millis: f64) -> Self {
        Self { millis }
    }
}

impl Clock for FixedClock {
    fn now_millis(&self) -> f64 {
        self.millis
    }
}
