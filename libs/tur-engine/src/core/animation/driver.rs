//! `ImplicitDriver` — the single shared timeline that drives an implicit
//! animation (`AnimatedContainer` / `AnimatedOpacity` / `AnimatedPositioned`).
//!
//! Mirrors Flutter's `ImplicitlyAnimatedWidget`, which uses one internal
//! `AnimationController` per widget regardless of how many properties are
//! animating. All animated props of a single element share this timeline:
//! they each hold their own `begin`/`end` ([`super::props::AnimatedProp`])
//! and interpolate at the driver's eased `t`.
//!
//! Lifecycle:
//!   - [`ImplicitDriver::retarget`] is called from the element's `Effect`
//!     phase when a target property changes. It does NOT have wall-clock
//!     access there, so it sets `pending_restart` and the timeline is stamped
//!     on the next [`ImplicitDriver::tick`].
//!   - [`ImplicitDriver::tick`] is called once per animation frame by
//!     [`super::AnimationManager::tick_drivers`], which has the clock. It
//!     computes the eased progress and reports completion (so the manager can
//!     fire `onEnd` once and stop marking the element dirty).

use tur_shared::Curve;

/// The result of one driver tick.
#[derive(Clone, Copy, Debug)]
pub struct TickOutcome {
    /// Eased progress in `[0, 1]` to feed each prop's `Tween::lerp`.
    pub eased_t: f64,
    /// `true` only on the tick that crosses the end. The manager fires
    /// `onEnd` exactly once per retarget when this is `true`.
    pub just_completed: bool,
}

/// A single shared animation timeline. Owned by [`super::AnimationManager`]
/// (one per animated element). The element reads the latest eased `t` via a
/// shared `Rc<Cell<f64>>` updated by the manager each tick.
#[derive(Debug, Clone)]
pub struct ImplicitDriver {
    duration_ms: u64,
    curve: Curve,
    /// Wall-clock ms when this segment started. Set by `start_at` (called
    /// from the manager's `retarget`, which has a cached clock reading).
    start_time_ms: Option<u64>,
    /// Whether the timeline is currently advancing. Cleared on completion.
    active: bool,
}

impl ImplicitDriver {
    pub fn new(duration_ms: u64, curve: Curve) -> Self {
        ImplicitDriver {
            duration_ms,
            curve,
            start_time_ms: None,
            active: false,
        }
    }

    /// Immediately stamp `start_time = now` and begin advancing. Called by
    /// [`super::AnimationManager::retarget`] using the manager's cached clock
    /// so the timeline starts on the retarget frame (not the next one).
    pub fn start_at(&mut self, now_ms: u64) {
        self.start_time_ms = Some(now_ms);
        self.active = true;
    }

    /// Advance the timeline by the elapsed wall-clock since `start_time_ms`.
    /// Returns `Some(outcome)` if the timeline is active; `None` when idle.
    pub fn tick(&mut self, now_ms: u64) -> Option<TickOutcome> {
        if !self.active {
            return None;
        }
        let start = self.start_time_ms?;
        let elapsed = now_ms.saturating_sub(start) as f64;
        let denom = self.duration_ms.max(1) as f64;
        let raw_t = (elapsed / denom).clamp(0.0, 1.0);
        let eased_t = self.curve.transform(raw_t);

        let just_completed = raw_t >= 1.0;
        if just_completed {
            self.active = false;
        }
        Some(TickOutcome {
            eased_t,
            just_completed,
        })
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}
