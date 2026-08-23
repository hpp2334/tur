use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use crate::core::layout::Offset;
use crate::core::shell::Cursor;
use boa_engine::context::time::Clock;

/// Per-frame cursor-claim accumulator written during the paint walk.
///
/// Each `MouseRegion` that the pointer is over calls [`CursorSink::set`] with
/// its resolved cursor. Because paint runs shallow→deep (root→leaf), the
/// deepest painted region writes last, so the final value is the innermost
/// claim. An opaque `MouseRegion` under the pointer calls `set(Cursor::Default)`
/// to drop any claims already written by its ancestors (which painted earlier).
///
/// `Cursor` is `Copy`, so the sink is a cheap shared `Cell`. It lives in
/// [`FrameEnv`] and is reset implicitly each frame by
/// `apply_cursor_changes` (which calls `take`). [`PaintEnv::set_cursor`]
/// writes through it.
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

/// Owner of per-frame environment state (clock, pointer position, cursor
/// resolution) and the privileged driver operations.
///
/// Held by the app driver (`TurAppContext`). The biz (paint / views) never
/// sees this type — only the [`PaintEnv`] face borrowed via
/// [`paint_env`].
///
/// `apply_cursor_changes` is the privileged post-paint flush: it resolves
/// the deepest cursor claim and dedups against the last applied value.
/// It is pure state — the *application* happens on the host thread: the
/// worker loop ships each change as `HostMsg::Shell(SetCursor)` and the
/// per-instance backend installed via
/// [`TurAppBuilder::shell`](crate::core::shell::Shell)
/// applies it. Biz cannot call it.
///
/// [`paint_env`]: FrameEnv::paint_env
pub struct FrameEnv {
    clock: Rc<dyn Clock>,
    pointer_position: Option<Offset>,
    cursor: CursorSink,
    applied_cursor: Option<Cursor>,
}

impl FrameEnv {
    pub fn new(clock: Rc<dyn Clock>) -> Self {
        FrameEnv {
            clock,
            pointer_position: None,
            cursor: CursorSink::new(),
            applied_cursor: None,
        }
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

    /// The shared clock handle. Plugins obtain this via
    /// [`PluginRegisterContext::clock`](crate::core::plugin::PluginRegisterContext::clock)
    /// and stash it in time-driven subsystems (animation, audio, …) so they
    /// can query `clock.now()` during their tick.
    pub fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }

    /// Record the latest pointer position (canvas-local logical pixels), or
    /// `None` to indicate no pointer is present. Called by the event layer on
    /// `PointerMove`.
    pub fn set_pointer_position(&mut self, position: Option<Offset>) {
        self.pointer_position = position;
    }

    /// Flush the cursor claims accumulated during paint: resolve the
    /// deepest-wins value and dedup against the last applied cursor.
    /// Called once by the driver after the paint pass. `take` empties the
    /// sink so the next frame starts clean (no separate reset needed).
    ///
    /// Pure state — the worker loop ships the deduped change (see
    /// `last_applied_cursor`) to the host thread, where the embedder's
    /// [`Shell`](crate::core::shell::Shell) applies it.
    pub fn apply_cursor_changes(&mut self) {
        let resolved = self.cursor.take().unwrap_or_default();
        let present = self.pointer_position.is_some();
        if present {
            self.applied_cursor = Some(resolved);
        }
    }

    /// The most recent cursor resolved via `apply_cursor_changes` (or `None`
    /// if no pointer position was ever recorded). Used by the worker loop to
    /// ship cursor changes to the host thread via
    /// `HostMsg::Shell(ShellCommand::SetCursor)`.
    pub fn last_applied_cursor(&self) -> Option<Cursor> {
        self.applied_cursor
    }

    /// Borrow the biz/paint face for one paint pass.
    pub fn paint_env(&self) -> PaintEnv<'_> {
        PaintEnv { inner: self }
    }
}

/// The face the biz (paint / `MouseRegion` / `PaintContext`) sees.
///
/// Constructed only via `FrameEnv::paint_env`. It exposes claiming a
/// cursor plus reading time and pointer position — but **not** the
/// privileged `apply_cursor_changes` / `set_pointer_position`, so biz
/// cannot flush or mutate driver state.
#[derive(Clone, Copy)]
pub struct PaintEnv<'a> {
    inner: &'a FrameEnv,
}

impl<'a> PaintEnv<'a> {
    /// Claim the host cursor for this frame. May be called many times during
    /// one paint pass (deepest painted region wins). Nothing is committed to
    /// the host until the driver flushes (`FrameEnv::apply_cursor_changes`).
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
