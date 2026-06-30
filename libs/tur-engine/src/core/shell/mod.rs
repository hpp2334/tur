use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use boa_engine::context::time::FixedClock;
use boa_engine::context::Clock;
use tur_shared::{Cursor, Offset};

use crate::core::platform_api::PlatformApi;

/// Per-frame cursor-claim accumulator written during the paint walk.
///
/// Each `MouseRegion` that the pointer is over calls [`CursorSink::set`] with
/// its resolved cursor. Because paint runs shallow→deep (root→leaf), the
/// deepest painted region writes last, so the final value is the innermost
/// claim. An opaque `MouseRegion` under the pointer calls `set(Cursor::Default)`
/// to drop any claims already written by its ancestors (which painted earlier).
///
/// `Cursor` is `Copy`, so the sink is a cheap shared `Cell`. It lives in
/// [`Shell`] and is reset implicitly each frame by `apply_changes`
/// (which calls `take`). [`PaintShell::set_cursor`] writes through it.
#[derive(Clone)]
pub(crate) struct CursorSink(Rc<Cell<Option<Cursor>>>);

impl CursorSink {
    pub(crate) fn new() -> Self {
        Self(Rc::new(Cell::new(None)))
    }

    /// Claim the cursor for this frame. Last write wins (deepest painted
    /// region, since paint is shallow→deep).
    pub(crate) fn set(&self, cursor: Cursor) {
        self.0.set(Some(cursor));
    }

    /// Take the resolved cursor, leaving the sink empty for the next frame.
    pub(crate) fn take(&self) -> Option<Cursor> {
        self.0.take()
    }
}

impl Default for CursorSink {
    fn default() -> Self {
        Self::new()
    }
}

/// Owner of shell state and the privileged driver operations.
///
/// Held by the app driver (`TurAppContext`). The biz (paint / views) never
/// sees this type — only the [`PaintShell`] face borrowed via [`paint_face`].
///
/// `apply_changes` is the privileged post-paint flush: it resolves the
/// deepest cursor claim, dedups against the last applied value, and pushes the
/// result to the embedder through [`PlatformApi::set_cursor`]. Biz cannot call it.
///
/// [`paint_face`]: Shell::paint_face
pub(crate) struct Shell {
    clock: Rc<FixedClock>,
    platform_api: Box<dyn PlatformApi>,
    pointer_position: Option<Offset>,
    cursor: CursorSink,
    applied_cursor: Option<Cursor>,
}

impl Shell {
    pub(crate) fn new(clock: Rc<FixedClock>, platform_api: Box<dyn PlatformApi>) -> Self {
        Shell {
            clock,
            platform_api,
            pointer_position: None,
            cursor: CursorSink::new(),
            applied_cursor: None,
        }
    }

    /// Advance the shared clock. The boa `Context` holds a clone of the same
    /// `Rc<FixedClock>`, so JS `Date.now()` advances in lockstep.
    pub(crate) fn forward(&self, ms: u64) {
        self.clock.forward(ms);
    }

    /// Current frame time as a `Duration` since the epoch.
    pub(crate) fn now(&self) -> Duration {
        Duration::from_millis(self.clock.now().millis_since_epoch())
    }

    /// Record the latest pointer position (canvas-local logical pixels), or
    /// `None` to indicate no pointer is present. Called by the event layer on
    /// `PointerMove`.
    pub(crate) fn set_pointer_position(&mut self, position: Option<Offset>) {
        self.pointer_position = position;
    }

    /// Flush the cursor claims accumulated during paint: resolve the
    /// deepest-wins value, dedup against the last applied cursor, and on change
    /// push it to the embedder via [`PlatformApi::set_cursor`]. Called once by the
    /// driver after the paint pass. `take` empties the sink so the next frame
    /// starts clean (no separate reset needed).
    pub(crate) fn apply_changes(&mut self) {
        let resolved = self.cursor.take().unwrap_or_default();
        let present = self.pointer_position.is_some();
        if present && self.applied_cursor != Some(resolved) {
            self.applied_cursor = Some(resolved);
            self.platform_api.set_cursor(resolved);
        }
    }

    /// Borrow the biz/paint face for one paint pass.
    pub(crate) fn paint_face(&self) -> PaintShell<'_> {
        PaintShell { inner: self }
    }
}

/// The face the biz (paint / `MouseRegion` / `PaintContext`) sees.
///
/// Constructed only via `Shell::paint_face`. It exposes claiming a
/// cursor plus reading time and pointer position — but **not** the privileged
/// `apply_changes` / `set_pointer_position` / `forward`, so biz cannot flush
/// or mutate driver state.
#[derive(Clone, Copy)]
pub struct PaintShell<'a> {
    inner: &'a Shell,
}

impl<'a> PaintShell<'a> {
    /// Claim the host cursor for this frame. May be called many times during
    /// one paint pass (deepest painted region wins). Nothing is committed to
    /// the host until the driver flushes (`Shell::apply_changes`).
    pub fn set_cursor(&self, cursor: Cursor) {
        self.inner.cursor.set(cursor);
    }

    /// Current frame time as a `Duration` since the epoch.
    pub fn now(&self) -> Duration {
        self.inner.now()
    }

    /// Last known pointer position, or `None` if no pointer move was received.
    pub fn pointer_position(&self) -> Option<Offset> {
        self.inner.pointer_position
    }
}
