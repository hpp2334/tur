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

use tur_engine::core::element::ElementNodeId;
use tur_shared::Offset;

const TOUCH_SLOP: f64 = 18.0;

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
    /// Scroll was resolved. Nothing to dispatch.
    ScrollEnded,
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
        });
    }

    /// Called on `PointerMove { device: Touch }`. Returns the outcome
    /// telling the handler what to dispatch.
    pub fn on_touch_move(&mut self, position: Offset) -> TouchMoveOutcome {
        let Some(ts) = self.touch.as_mut() else {
            return TouchMoveOutcome::Idle;
        };

        let dx = position.x - ts.last_position.x;
        let dy = position.y - ts.last_position.y;
        ts.last_position = position;
        ts.move_seen = true;

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
            Some(ArenaWinnerKind::Scroll) => TouchUpOutcome::ScrollEnded,
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
