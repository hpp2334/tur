//! Gesture arena for touch-based drag/scroll disambiguation.
//!
//! Inspired by Flutter's gesture arena: when a touch-drag begins, multiple
//! elements in the hit-path may want to handle it — a `ScrollView` wants to
//! scroll, a `PointerInteract` wants to drag, an `EditableText` wants to
//! select text. The arena resolves this competition using a slop threshold
//! (`kTouchSlop` = 18 px, matching Flutter).
//!
//! The arena is a **pure slop tracker** — it does not query the element tree
//! or decide the winner. At slop-exceeding movement, it returns
//! `SlopExceeded` and the handler probes gesture elements by dispatching
//! `PointerDown { device: Touch }`. If the element returns `true` (claims),
//! the handler calls `resolve(Drag)`. If no element claims, the handler
//! calls `resolve(Scroll)` and pushes wheel events.
//!
//! Mouse events bypass the arena entirely (immediate dispatch, no slop).

use crate::core::element::ElementNodeId;
use crate::core::layout::Offset;

const TOUCH_SLOP: f64 = 18.0;

/// Maximum total displacement (px, from the down position to the release
/// position) for a touch sequence to qualify as a tap. Mirrors `TOUCH_SLOP`:
/// movement under this never resolves the arena to drag/scroll, so a release
/// here is a tap (not a drag that stayed sub-slop).
const TAP_MAX_DISTANCE_PX: f64 = TOUCH_SLOP;

/// Maximum duration (ms, down→up) for a touch sequence to qualify as a tap.
/// A long press-and-release that stayed under slop should not fire a click.
/// 500 ms matches the multi-click window in `composer.rs` and the typical
/// long-press threshold on mobile platforms.
const TAP_MAX_DURATION_MS: u64 = 500;

/// Window (ms) of recent touch samples used to estimate fling velocity.
/// Samples older than this are pruned. 100 ms matches the typical "last
/// gesture snippet" window used by native velocity trackers (Android /
/// Flutter).
const VELOCITY_WINDOW_MS: u64 = 100;

/// Determinant magnitude below which the degree-2 least-squares system is
/// considered singular (e.g. all samples share the same timestamp) and we
/// fall back to a linear fit.
const SINGULAR_EPS: f64 = 1e-9;

/// Minimum span (ms) — represented by the count of distinct timestamps —
/// required to produce a velocity estimate. Below this, the tracker reports
/// zero (no reliable time signal).
const MIN_DISTINCT_TIMES: usize = 2;

/// Recent touch samples (position + time) kept in a sliding window to
/// estimate the drag velocity at release.
///
/// Uses a **degree-2 least-squares polynomial fit** of x(t) and y(t) over the
/// window (Flutter's `_LeastSquaresVelocityTrackerStrategy`), then returns
/// the derivative (tangent velocity) at the latest sample — i.e. the velocity
/// at the moment of release. This uses *every* sample, so it is robust to a
/// single slow/stale sample (the failure mode of a two-point estimate) and to
/// several touchmoves sharing a timestamp (mobile coalescing). Falls back to a
/// degree-1 (linear) fit when there are too few distinct timestamps or the
/// quadratic system is singular.
#[derive(Default)]
struct VelocityTracker {
    samples: Vec<(Offset, u64)>,
}

impl VelocityTracker {
    fn record(&mut self, position: Offset, time_ms: u64) {
        // Prune samples older than the window.
        let cutoff = time_ms.saturating_sub(VELOCITY_WINDOW_MS);
        let mut keep_from = 0;
        for (i, &(_, t)) in self.samples.iter().enumerate() {
            if t < cutoff {
                keep_from = i + 1;
            } else {
                break;
            }
        }
        if keep_from > 0 {
            self.samples.drain(0..keep_from);
        }
        self.samples.push((position, time_ms));
    }

    /// Returns `(vx, vy)` in touch-movement px/ms — the tangent velocity at
    /// the most recent sample. `(0, 0)` if there isn't enough time span in the
    /// window.
    fn velocity_px_per_ms(&self) -> (f64, f64) {
        if self.samples.len() < 2 {
            return (0.0, 0.0);
        }
        // Time relative to the oldest sample (keeps `t` small → the 3×3
        // normal-equation matrix stays well-conditioned).
        let t0 = self.samples.first().unwrap().1;
        let pts: Vec<(f64, f64, f64)> = self
            .samples
            .iter()
            .map(|(p, t)| ((t.saturating_sub(t0)) as f64, p.x, p.y))
            .collect();

        let distinct_times = count_distinct_times(&pts);
        let t_last = pts.last().unwrap().0;

        if distinct_times >= 3
            && let Some((vx, vy)) = fit_quadratic(&pts, t_last)
        {
            return (vx, vy);
        }
        if distinct_times >= MIN_DISTINCT_TIMES {
            return linear_slope(&pts);
        }
        (0.0, 0.0)
    }
}

/// Number of distinct timestamps in the window (samples sharing a timestamp
/// — e.g. several touchmoves coalesced into one frame — count once).
fn count_distinct_times(pts: &[(f64, f64, f64)]) -> usize {
    let mut sorted: Vec<f64> = pts.iter().map(|(t, _, _)| *t).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sorted.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    sorted.len()
}

/// Determinant of a 3×3 matrix (row-major).
fn det3(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Degree-2 least-squares fit of x(t) = a·t² + b·t + c (and likewise for y),
/// solved via the normal equations + Cramer's rule. Returns the tangent
/// velocity `(dx/dt, dy/dt)` at `t_eval`. `None` if the system is singular.
fn fit_quadratic(pts: &[(f64, f64, f64)], t_eval: f64) -> Option<(f64, f64)> {
    let (mut s1, mut s2, mut s3, mut s4) = (0.0, 0.0, 0.0, 0.0);
    let (mut sx, mut sx1, mut sx2) = (0.0, 0.0, 0.0);
    let (mut sy, mut sy1, mut sy2) = (0.0, 0.0, 0.0);
    let n = pts.len() as f64;
    for &(t, x, y) in pts {
        let t2 = t * t;
        let t3 = t2 * t;
        let t4 = t3 * t;
        s1 += t;
        s2 += t2;
        s3 += t3;
        s4 += t4;
        sx += x;
        sx1 += t * x;
        sx2 += t2 * x;
        sy += y;
        sy1 += t * y;
        sy2 += t2 * y;
    }
    // Normal-equation matrix (symmetric): rows = [t², t, 1] basis.
    let m = [[s4, s3, s2], [s3, s2, s1], [s2, s1, n]];
    let det = det3(&m);
    if det.abs() < SINGULAR_EPS {
        return None;
    }
    // Cramer's rule: replace the column for the coefficient being solved.
    // `a` (t² coeff) → column 0, `b` (t coeff) → column 1. (`c` isn't needed
    // for the derivative.)
    let ax = det3(&[[sx2, s3, s2], [sx1, s2, s1], [sx, s1, n]]) / det;
    let bx = det3(&[[s4, sx2, s2], [s3, sx1, s1], [s2, sx, n]]) / det;
    let ay = det3(&[[sy2, s3, s2], [sy1, s2, s1], [sy, s1, n]]) / det;
    let by = det3(&[[s4, sy2, s2], [s3, sy1, s1], [s2, sy, n]]) / det;
    // d/dt (a·t² + b·t + c) = 2a·t + b.
    Some((2.0 * ax * t_eval + bx, 2.0 * ay * t_eval + by))
}

/// Degree-1 (linear) least-squares slope — the average velocity over the
/// window. Robust fallback that still uses every sample.
fn linear_slope(pts: &[(f64, f64, f64)]) -> (f64, f64) {
    let n = pts.len() as f64;
    let (sum_t, sum_x, sum_y) = pts.iter().fold((0.0, 0.0, 0.0), |(st, sx, sy), (t, x, y)| {
        (st + t, sx + x, sy + y)
    });
    let (tbar, xbar, ybar) = (sum_t / n, sum_x / n, sum_y / n);
    let (mut num_x, mut num_y, mut den) = (0.0, 0.0, 0.0);
    for (t, x, y) in pts {
        let dt = t - tbar;
        num_x += dt * (x - xbar);
        num_y += dt * (y - ybar);
        den += dt * dt;
    }
    if den.abs() < SINGULAR_EPS {
        (0.0, 0.0)
    } else {
        (num_x / den, num_y / den)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ArenaWinnerKind {
    Drag,
    Scroll,
}

pub enum TouchMoveOutcome {
    /// Movement below slop; no dispatch needed.
    Idle,
    /// Slop exceeded for the first time. The handler should walk the
    /// hit-path, dispatch `PointerDown { device: Touch }` to each gesture
    /// element until one claims (returns `true`). If none claims, resolve
    /// to scroll. Then call `resolve()` with the result.
    SlopExceeded,
    /// Drag already resolved. Dispatch `PointerMove` to the captured path.
    DragMoved,
    /// Scroll already resolved. Push an `AppEvent::Wheel` with the delta.
    Scroll {
        delta_x: f64,
        delta_y: f64,
        position: Offset,
    },
}

pub enum TouchUpOutcome {
    /// Drag was resolved. Dispatch `PointerUp` to the captured path and
    /// release gesture capture.
    DragEnded,
    /// Scroll was resolved. Nothing to dispatch. Carries the tracked
    /// touch-movement velocity (px/ms) so the handler can seed a fling
    /// simulation. `(0, 0)` if there wasn't enough movement to estimate.
    ScrollEnded { vx: f64, vy: f64 },
    /// The touch sequence ended without resolving to drag or scroll, and it
    /// qualifies as a tap (short duration, movement stayed within slop). The
    /// handler synthesizes a mouse `PointerDown` + `PointerUp` to fire
    /// click/focus — the engine owns tap→click synthesis on every host
    /// (browser, Android, desktop), so embedders must NOT also forward a
    /// browser-synthesized click for the same tap (double dispatch).
    /// `position`/`time_ms` are the **down** position + **down** time, used
    /// for multi-click classification.
    Tap { position: Offset, time_ms: u64 },
    /// The touch sequence ended without resolving and does NOT qualify as a
    /// tap (too long, or moved beyond slop without crossing the drag
    /// threshold — e.g. a long-press-and-release). Nothing to dispatch.
    Idle,
}

pub enum TouchCancelOutcome {
    /// Drag was resolved. Dispatch `PointerUp` to release capture.
    DragCanceled,
    /// Nothing was resolved. Nothing to dispatch.
    Idle,
}

struct TouchState {
    down_position: Offset,
    last_position: Offset,
    time_ms: u64,
    hit_path: Vec<ElementNodeId>,
    winner: Option<ArenaWinnerKind>,
    /// Recent touch-move samples used to estimate the drag velocity at
    /// release, so a scroll-resolved drag can seed an inertia fling.
    velocity: VelocityTracker,
}

pub struct GestureArena {
    touch: Option<TouchState>,
}

impl Default for GestureArena {
    fn default() -> Self {
        Self::new()
    }
}

impl GestureArena {
    pub fn new() -> Self {
        Self { touch: None }
    }

    /// Called on `PointerDown { device: Touch }`. Records the hit-path
    /// for later probing. Does NOT dispatch anything. Also seeds the velocity
    /// tracker with the down position so the fling estimate has a baseline
    /// (Flutter records every pointer event, including down).
    pub fn on_touch_down(&mut self, position: Offset, time_ms: u64, hit_path: Vec<ElementNodeId>) {
        let mut velocity = VelocityTracker::default();
        velocity.record(position, time_ms);
        self.touch = Some(TouchState {
            down_position: position,
            last_position: position,
            time_ms,
            hit_path,
            winner: None,
            velocity,
        });
    }

    /// Called on `PointerMove { device: Touch }`. Returns the outcome
    /// telling the handler what to dispatch. `now_ms` is the event's own
    /// timestamp (carried on `PointerMove` from the platform, e.g. the
    /// browser's `event.timeStamp`) and feeds the velocity tracker so fling
    /// velocity can be estimated on release.
    pub fn on_touch_move(&mut self, position: Offset, now_ms: u64) -> TouchMoveOutcome {
        let Some(ts) = self.touch.as_mut() else {
            return TouchMoveOutcome::Idle;
        };

        let dx = position.x - ts.last_position.x;
        let dy = position.y - ts.last_position.y;
        ts.last_position = position;
        ts.velocity.record(position, now_ms);

        if let Some(winner) = ts.winner {
            return match winner {
                ArenaWinnerKind::Drag => TouchMoveOutcome::DragMoved,
                ArenaWinnerKind::Scroll => TouchMoveOutcome::Scroll {
                    delta_x: -dx,
                    delta_y: -dy,
                    position,
                },
            };
        }

        let movement_x = position.x - ts.down_position.x;
        let movement_y = position.y - ts.down_position.y;
        let distance = (movement_x * movement_x + movement_y * movement_y).sqrt();
        if distance < TOUCH_SLOP {
            return TouchMoveOutcome::Idle;
        }

        TouchMoveOutcome::SlopExceeded
    }

    /// Called by the handler after probing gesture elements at
    /// `SlopExceeded`. Tells the arena whether drag or scroll won so
    /// subsequent moves are routed correctly.
    pub fn resolve(&mut self, kind: ArenaWinnerKind) {
        if let Some(ts) = self.touch.as_mut() {
            ts.winner = Some(kind);
        }
    }

    /// Called on `PointerUp { device: Touch }`. Records a final sample at the
    /// release position so the velocity estimate reflects the moment of
    /// lift-off, then computes the fling velocity. If the sequence did not
    /// resolve to drag/scroll, classifies it as a tap (short + sub-slop) or
    /// idle (too long / too far). The engine synthesizes the click for a tap
    /// on every host — see `TouchUpOutcome::Tap`.
    pub fn on_touch_up(&mut self, position: Offset, time_ms: u64) -> TouchUpOutcome {
        let Some(mut ts) = self.touch.take() else {
            return TouchUpOutcome::Idle;
        };
        ts.velocity.record(position, time_ms);
        match ts.winner {
            Some(ArenaWinnerKind::Drag) => TouchUpOutcome::DragEnded,
            Some(ArenaWinnerKind::Scroll) => {
                let (vx, vy) = ts.velocity.velocity_px_per_ms();
                TouchUpOutcome::ScrollEnded { vx, vy }
            }
            None => {
                // No drag/scroll won. Decide tap vs idle from the gesture's
                // total duration and displacement — the same gesture is a tap
                // whether or not the finger jittered, so the former
                // `move_seen` split (which assumed a host browser would
                // synthesize the click for the no-move case) is gone. The
                // engine synthesizes the click itself for any qualifying tap.
                let dx = position.x - ts.down_position.x;
                let dy = position.y - ts.down_position.y;
                let distance = (dx * dx + dy * dy).sqrt();
                let duration = time_ms.saturating_sub(ts.time_ms);
                if distance <= TAP_MAX_DISTANCE_PX && duration <= TAP_MAX_DURATION_MS {
                    TouchUpOutcome::Tap {
                        position: ts.down_position,
                        time_ms: ts.time_ms,
                    }
                } else {
                    TouchUpOutcome::Idle
                }
            }
        }
    }

    /// Called on `PointerCancel { device: Touch }`.
    pub fn on_touch_cancel(&mut self) -> TouchCancelOutcome {
        let Some(ts) = self.touch.take() else {
            return TouchCancelOutcome::Idle;
        };
        match ts.winner {
            Some(ArenaWinnerKind::Drag) => TouchCancelOutcome::DragCanceled,
            _ => TouchCancelOutcome::Idle,
        }
    }

    /// The hit-path captured at touch-down. Available after `on_touch_down`.
    pub fn touch_hit_path(&self) -> Option<&[ElementNodeId]> {
        self.touch.as_ref().map(|ts| ts.hit_path.as_slice())
    }

    /// The position where the touch started.
    pub fn touch_down_position(&self) -> Option<Offset> {
        self.touch.as_ref().map(|ts| ts.down_position)
    }
}
