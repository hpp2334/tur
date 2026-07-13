use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use boa_engine::context::time::Clock;
use tur_shared::{Cursor, Offset};

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
pub struct CursorSink(Rc<Cell<Option<Cursor>>>);

impl CursorSink {
    pub fn new() -> Self {
        Self(Rc::new(Cell::new(None)))
    }

    /// Claim the cursor for this frame. Last write wins (deepest painted
    /// region, since paint is shallow→deep).
    pub fn set(&self, cursor: Cursor) {
        self.0.set(Some(cursor));
    }

    /// Take the resolved cursor, leaving the sink empty for the next frame.
    pub fn take(&self) -> Option<Cursor> {
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
/// result through the `cursor_output` callback (installed by a plugin at
/// build time). Biz cannot call it.
///
/// [`paint_face`]: Shell::paint_face
pub struct Shell {
    clock: Rc<dyn Clock>,
    cursor_output: Option<Box<dyn FnMut(Cursor)>>,
    pointer_position: Option<Offset>,
    cursor: CursorSink,
    applied_cursor: Option<Cursor>,
    /// Absolute deadline (epoch-relative, same as `now()`) at which the
    /// engine should force a paint-only redraw. Populated by elements
    /// during paint via [`PaintShell::request_redraw_after`]; cleared at
    /// the start of each paint pass so only the current pass's requests
    /// survive. Read by `flush()` after paint for `NextFrame` scheduling.
    redraw_deadline: Cell<Option<Duration>>,
}

impl Shell {
    pub fn new(clock: Rc<dyn Clock>) -> Self {
        Shell {
            clock,
            cursor_output: None,
            pointer_position: None,
            cursor: CursorSink::new(),
            applied_cursor: None,
            redraw_deadline: Cell::new(None),
        }
    }

    /// Install a cursor-output callback. Called at build time by the engine
    /// builder when a plugin provides `cursor_output()`. The callback fires
    /// at runtime during `apply_changes` whenever the resolved cursor changes.
    pub fn set_cursor_output(&mut self, f: Box<dyn FnMut(Cursor)>) {
        self.cursor_output = Some(f);
    }

    /// Current frame time as a `Duration` since the epoch.
    ///
    /// The clock is shared with the boa `Context` (the same `Rc<dyn Clock>`
    /// is passed to both at build time), so JS `Date.now()` and engine
    /// scheduling read the same source. The clock is advanced by the embedder
    /// — a real wall-clock (`StdClock`) in production self-advances; a
    /// `FixedClock` in tests is bumped by the test harness.
    pub fn now(&self) -> Duration {
        Duration::from_millis(self.clock.now().millis_since_epoch())
    }

    /// Record the latest pointer position (canvas-local logical pixels), or
    /// `None` to indicate no pointer is present. Called by the event layer on
    /// `PointerMove`.
    pub fn set_pointer_position(&mut self, position: Option<Offset>) {
        self.pointer_position = position;
    }

    /// Flush the cursor claims accumulated during paint: resolve the
    /// deepest-wins value, dedup against the last applied cursor, and on change
    /// invoke the `cursor_output` callback (installed by a plugin). Called once
    /// by the driver after the paint pass. `take` empties the sink so the next
    /// frame starts clean (no separate reset needed).
    pub fn apply_changes(&mut self) {
        let resolved = self.cursor.take().unwrap_or_default();
        let present = self.pointer_position.is_some();
        if present && self.applied_cursor != Some(resolved) {
            self.applied_cursor = Some(resolved);
            if let Some(f) = &mut self.cursor_output {
                f(resolved);
            }
        }
    }

    /// Borrow the biz/paint face for one paint pass.
    pub fn paint_face(&self) -> PaintShell<'_> {
        PaintShell { inner: self }
    }

    /// Read the pending redraw deadline without consuming it. Returns an
    /// absolute `Duration` (epoch-relative, comparable to `now()`), or
    /// `None` if no element requested a timed redraw during the last paint.
    pub fn peek_redraw_deadline(&self) -> Option<Duration> {
        self.redraw_deadline.get()
    }

    /// Clear the redraw deadline. Called at the start of each paint pass
    /// so only requests made during the current pass survive.
    pub fn clear_redraw_deadline(&self) {
        self.redraw_deadline.set(None);
    }
}

/// The face the biz (paint / `MouseRegion` / `PaintContext`) sees.
///
/// Constructed only via `Shell::paint_face`. It exposes claiming a
/// cursor plus reading time and pointer position — but **not** the privileged
/// `apply_changes` / `set_pointer_position`, so biz cannot flush
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

    /// Request a paint-only redraw after `delay` from now. Multiple calls
    /// during one paint pass take the earliest deadline. The engine reads
    /// the accumulated deadline after paint to schedule the next wake-up
    /// and force a paint when the deadline expires.
    pub fn request_redraw_after(&self, delay: Duration) {
        let deadline_ms = self.inner.clock.now().millis_since_epoch()
            + delay.as_millis() as u64;
        let deadline = Duration::from_millis(deadline_ms);
        let prev = self.inner.redraw_deadline.get();
        let next = match prev {
            None => Some(deadline),
            Some(d) => Some(d.min(deadline)),
        };
        self.inner.redraw_deadline.set(next);
    }
}
