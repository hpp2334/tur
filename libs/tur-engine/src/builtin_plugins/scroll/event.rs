//! Scroll inertia event payload travelling on the engine's app-event bus.
//!
//! The engine's [`AppEvent`](crate::core::app::AppEvent) enum carries only
//! the core scroll primitives plus a single `Custom` escape hatch. This
//! module defines the scroll-inertia payload produced by the gesture plugin
//! (on touch-up after a scroll-resolved drag) and consumed by the scroll
//! plugin's [`ScrollInertiaSubsystem`](crate::builtin_plugins::scroll::handlers::ScrollInertiaSubsystem),
//! so the engine core never needs to know about inertia.
//!
//! - **Producer**: `GestureSubsystem` in `builtin_plugins/gesture` — on
//!   `ScrollEnded` it reads the velocity tracked by the arena and pushes a
//!   [`ScrollFlingEvent`] via [`push_fling`]. Cross-plugin import via
//!   `pub(in crate::builtin_plugins)`.
//! - **Consumer**: `ScrollInertiaSubsystem` in
//!   `builtin_plugins/scroll/handlers` — seeds an exponential-decay
//!   integrator from the velocity and pushes `AppEvent::Scroll` deltas each
//!   frame until the velocity decays below threshold.

use crate::core::app::{AppEvent, AppEventQueue, CustomAppEvent};
use crate::core::layout::Offset;

/// Touch-fling seed produced by the gesture arena on touch-up after a
/// scroll-resolved drag. Carries the scroll-delta velocity (px/ms, already
/// negated relative to touch-movement direction so positive = scroll
/// right/down, matching [`AppEvent::Scroll`]'s delta convention) plus the
/// position at which the fling should be hit-tested each frame.
#[derive(Debug)]
pub struct ScrollFlingEvent {
    /// Scroll-delta velocity along x, px/ms.
    pub vx: f64,
    /// Scroll-delta velocity along y, px/ms.
    pub vy: f64,
    /// Hit-test position (the touch-up location). Re-used each frame to
    /// route inertia deltas through the same wheel pipeline as live scroll.
    pub position: Offset,
}

impl CustomAppEvent for ScrollFlingEvent {
    fn name(&self) -> &'static str {
        "scroll.fling"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Push a scroll-fling seed onto `queue`. Used by `GestureSubsystem` on
/// touch-up after a scroll-resolved drag.
pub(in crate::builtin_plugins) fn push_fling(
    queue: &mut AppEventQueue,
    vx: f64,
    vy: f64,
    position: Offset,
) {
    queue.push(AppEvent::custom(ScrollFlingEvent { vx, vy, position }));
}
