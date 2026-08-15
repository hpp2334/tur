//! Touch-scroll inertia (kinetic / fling scrolling).
//!
//! After a scroll-resolved touch drag ends, the gesture arena pushes a
//! [`ScrollFlingEvent`](crate::builtin_plugins::scroll::event::ScrollFlingEvent)
//! carrying the drag's final velocity. This subsystem seeds an
//! exponential-decay integrator from that velocity and emits
//! [`AppEvent::Scroll`](crate::core::app::AppEvent::Scroll) deltas each
//! frame, routing through the same wheel pipeline as live scroll so
//! hit-testing, clamping, overscroll chaining, and `onScroll` callbacks all
//! behave identically.
//!
//! Cancellation: any fresh touch (`PointerDown { device: Touch }`) or
//! touch-cancel immediately clears the active fling — touching the screen
//! stops the coast, matching native behaviour.
//!
//! The simulation uses an exponential-decay model `v(t) = v0 * exp(-t/tau)`
//! with `tau ≈ 325 ms` (tunable). The exact integral over each frame is
//! used to compute the per-frame delta: `Δx = v0 * tau * (1 - exp(-dt/tau))`.

use std::rc::Rc;

use boa_engine::context::time::Clock;

use crate::core::app::AppEvent;
use crate::core::platform::{PlatformEvent, PointerDeviceKind, PointerInput, ShellEventPayload};
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

use crate::builtin_plugins::scroll::event::ScrollFlingEvent;

/// Time constant for the exponential velocity decay. Higher = glides longer.
/// 325 ms is in the iOS / Flutter ballpark.
const TIME_CONSTANT_MS: f64 = 325.0;

/// Velocity magnitude (px/ms) below which the fling is considered stopped.
/// 0.05 px/ms = 50 px/s.
const VELOCITY_EPSILON_PX_PER_MS: f64 = 0.05;

/// Per-frame delta cap (ms). Bounds the integration step if the engine
/// paused for a long time (e.g. tab backgrounded) so the fling doesn't jump
/// absurdly in one frame.
const MAX_FRAME_MS: f64 = 50.0;

struct InertiaState {
    /// Scroll-delta velocity (px/ms). positive = scroll right/down.
    vx: f64,
    vy: f64,
    /// Hit-test position (touch-up location). Re-used each frame.
    position: crate::core::layout::Offset,
    /// The view root the fling targets (re-routing through its tree).
    root: crate::core::element::ViewRootId,
    /// Clock time at the last integration tick (ms since epoch).
    last_ms: u64,
}

/// Touch-scroll inertia subsystem. Captures the engine clock at registration
/// so it can integrate exponential decay each `flush`.
///
/// Registered by [`install_scroll`](crate::builtin_plugins::scroll::install_scroll)
/// after [`ScrollSubsystem`](super::ScrollSubsystem) so fling-seed events
/// (which arrive via `handle_app_event`) are processed after the gesture
/// plugin pushes them.
///
/// ## Flush frequency / self-gating
///
/// The engine calls [`Subsystem::flush`] **every fixed-point iteration** of a
/// `flush` call (possibly several times per frame). Integrating the decay,
/// however, must sample the clock **at most once per frame** — otherwise the
/// same frame would apply the delta multiple times. We self-gate that via
/// [`SubsystemFlushContext::frame_id`]: a per-`flush()` epoch stable across
/// the iterations of one frame and differing across frames.
pub struct ScrollInertiaSubsystem {
    clock: Rc<dyn Clock>,
    state: Option<InertiaState>,
    /// Last `frame_id` we integrated for. Stable across the fixed-point
    /// iterations of one `flush()` call, so repeated iterations within the
    /// same frame skip re-integration.
    last_frame: u64,
}

impl ScrollInertiaSubsystem {
    #[must_use]
    pub fn new(clock: Rc<dyn Clock>) -> Self {
        Self {
            clock,
            state: None,
            last_frame: 0,
        }
    }

    /// Returns `true` if there is no active fling. Used by tests.
    pub fn is_idle(&self) -> bool {
        self.state.is_none()
    }
}

impl Subsystem for ScrollInertiaSubsystem {
    fn handle_platform_event(
        &mut self,
        _cx: &mut SubsystemFlushContext<'_>,
        event: &PlatformEvent,
    ) {
        // Any fresh touch (down or cancel) stops the coast immediately —
        // the user touched the screen to grab the flinging content.
        if let ShellEventPayload::Pointer { input, .. } = event.payload() {
            let is_touch = match input {
                PointerInput::PointerDown { device, .. } => *device == PointerDeviceKind::Touch,
                PointerInput::PointerCancel { device } => *device == PointerDeviceKind::Touch,
                _ => false,
            };
            if is_touch {
                self.state = None;
            }
        }
    }

    fn handle_app_event(&mut self, _cx: &mut SubsystemFlushContext<'_>, event: &AppEvent) {
        if let Some(fling) = event.as_custom::<ScrollFlingEvent>() {
            // Ignore tiny flings — they wouldn't visibly move.
            let speed_sq = fling.vx * fling.vx + fling.vy * fling.vy;
            if speed_sq >= VELOCITY_EPSILON_PX_PER_MS * VELOCITY_EPSILON_PX_PER_MS {
                self.state = Some(InertiaState {
                    vx: fling.vx,
                    vy: fling.vy,
                    position: fling.position,
                    root: fling.root,
                    last_ms: self.clock.now().millis_since_epoch(),
                });
            }
        }
    }

    fn flush_pre_layout(&mut self, cx: &mut SubsystemFlushContext<'_>) {
        // Integrate the decay at most once per frame: only when the engine's
        // frame id has moved on since our last advance. `frame_id` is stable
        // across the fixed-point iterations of one `flush()` call, so repeated
        // iterations within the same frame skip re-integration (no double
        // delta for one frame).
        let id = cx.frame_id();
        if id != self.last_frame {
            self.last_frame = id;
            if let Some(state) = self.state.as_mut() {
                let now_ms = self.clock.now().millis_since_epoch();
                let dt_ms_raw = now_ms.saturating_sub(state.last_ms);
                if dt_ms_raw > 0 {
                    let dt_ms = (dt_ms_raw as f64).min(MAX_FRAME_MS);

                    // Exponential decay: v(t) = v0 * exp(-t/tau).
                    // Exact delta over [0, dt]:
                    //   ∫ v0*exp(-t/tau) dt = v0 * tau * (1 - exp(-dt/tau)).
                    let decay = (-dt_ms / TIME_CONSTANT_MS).exp();
                    let one_minus_decay = 1.0 - decay;
                    let delta_x = state.vx * TIME_CONSTANT_MS * one_minus_decay;
                    let delta_y = state.vy * TIME_CONSTANT_MS * one_minus_decay;

                    state.vx *= decay;
                    state.vy *= decay;
                    state.last_ms = now_ms;

                    // Route the delta through the same pipeline as live scroll
                    // so hit-testing, overscroll chaining, and `onScroll` all
                    // work. `ScrollSubsystem` (registered before us) drains
                    // this `AppEvent::Scroll` next iteration.
                    cx.app_event_queue.push(AppEvent::Scroll {
                        root: state.root,
                        delta_x,
                        delta_y,
                        position: state.position,
                    });
                    cx.mark_dirty();

                    // Stop when velocity drops below threshold.
                    let speed_sq = state.vx * state.vx + state.vy * state.vy;
                    if speed_sq < VELOCITY_EPSILON_PX_PER_MS * VELOCITY_EPSILON_PX_PER_MS {
                        self.state = None;
                    }
                }
            }
        }
        // Keep scheduling vsync while the fling is active. Emitted on every
        // iteration (cheap + idempotent) so a fling seeded mid-frame (from a
        // handler) still advances on the next frame.
        if self.state.is_some() {
            cx.request_next_frame();
        }
    }
}
