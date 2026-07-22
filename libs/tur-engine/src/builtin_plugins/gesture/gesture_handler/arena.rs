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

/// Window (ms) of recent touch-move samples used to estimate fling velocity.
/// Samples older than this are pruned. 100 ms matches the typical "last
/// gesture snippet" window used by native velocity trackers (Android /
/// Flutter).
const VELOCITY_WINDOW_MS: u64 = 100;

/// Minimum span (ms) between the oldest and newest sample in the window
/// required to produce a velocity estimate. Below this, the tracker reports
/// zero — too little signal to be reliable.
const VELOCITY_MIN_DT_MS: f64 = 2.0;

/// Recent touch-move samples (position + time) kept in a sliding window to
/// estimate the drag velocity at release. The oldest sample in the window
/// is paired with the newest to compute the average velocity.
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

    /// Returns `(vx, vy)` in touch-movement px/ms. `(0, 0)` if there isn't
    /// enough time span in the window.
    fn velocity_px_per_ms(&self) -> (f64, f64) {
        if self.samples.len() < 2 {
            return (0.0, 0.0);
        }
        let &(p0, t0) = self.samples.first().unwrap();
        let &(p1, t1) = self.samples.last().unwrap();
        let dt = (t1.saturating_sub(t0)) as f64;
        if dt < VELOCITY_MIN_DT_MS {
            return (0.0, 0.0);
        }
        ((p1.x - p0.x) / dt, (p1.y - p0.y) / dt)
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
    /// No resolution but `touchmove` was seen (small movement < slop).
    /// Synthesize a mouse `PointerDown` + `PointerUp` to fire click/focus.
    NeedsSyntheticClick {
        position: Offset,
        time_ms: u64,
    },
    /// No resolution and no `touchmove`. The browser synthesizes a click.
    BrowserClick,
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
    move_seen: bool,
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
    /// for later probing. Does NOT dispatch anything.
    pub fn on_touch_down(
        &mut self,
        position: Offset,
        time_ms: u64,
        hit_path: Vec<ElementNodeId>,
    ) {
        self.touch = Some(TouchState {
            down_position: position,
            last_position: position,
            time_ms,
            hit_path,
            winner: None,
            move_seen: false,
            velocity: VelocityTracker::default(),
        });
    }

    /// Called on `PointerMove { device: Touch }`. Returns the outcome
    /// telling the handler what to dispatch. `now_ms` is sampled by the
    /// caller (the gesture subsystem samples the engine clock) and feeds
    /// the velocity tracker so fling velocity can be estimated on release.
    pub fn on_touch_move(&mut self, position: Offset, now_ms: u64) -> TouchMoveOutcome {
        let Some(ts) = self.touch.as_mut() else {
            return TouchMoveOutcome::Idle;
        };

        let dx = position.x - ts.last_position.x;
        let dy = position.y - ts.last_position.y;
        ts.last_position = position;
        ts.move_seen = true;
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

    /// Called on `PointerUp { device: Touch }`.
    pub fn on_touch_up(&mut self) -> TouchUpOutcome {
        let Some(ts) = self.touch.take() else {
            return TouchUpOutcome::BrowserClick;
        };
        match ts.winner {
            Some(ArenaWinnerKind::Drag) => TouchUpOutcome::DragEnded,
            Some(ArenaWinnerKind::Scroll) => {
                let (vx, vy) = ts.velocity.velocity_px_per_ms();
                TouchUpOutcome::ScrollEnded { vx, vy }
            }
            None => {
                if ts.move_seen {
                    TouchUpOutcome::NeedsSyntheticClick {
                        position: ts.down_position,
                        time_ms: ts.time_ms,
                    }
                } else {
                    TouchUpOutcome::BrowserClick
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
