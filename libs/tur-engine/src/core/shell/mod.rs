use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::core::element::ViewRootId;
use crate::core::layout::Offset;
use crate::core::platform::Cursor;
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
/// [`Shell`] and is reset implicitly each frame by `apply_changes`
/// (which calls `take`). [`PaintShell::set_cursor`] writes through it.
#[derive(Clone)]
pub struct CursorSink(Rc<std::cell::Cell<Option<Cursor>>>);

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
/// deepest cursor claim, dedups against the last applied value, and pushes
/// the result through the installed `CursorBackend` (looked up from the
/// capability registry at `build()` time). Biz cannot call it.
///
/// [`paint_face`]: Shell::paint_face
pub struct Shell {
    clock: Rc<dyn Clock>,
    cursor_platform:
        Option<Arc<std::sync::Mutex<dyn crate::core::platform::CursorBackend + Send + Sync>>>,
    /// Per-root pointer positions (canvas-local logical pixels). Multi-root
    /// instances track one position per root; cursor claims resolve against
    /// the root being painted.
    pointer_positions: RefCell<HashMap<ViewRootId, Offset>>,
    cursor_sinks: RefCell<HashMap<ViewRootId, CursorSink>>,
    applied_cursors: RefCell<HashMap<ViewRootId, Cursor>>,
}

impl Shell {
    pub fn new(clock: Rc<dyn Clock>) -> Self {
        Shell {
            clock,
            cursor_platform: None,
            pointer_positions: RefCell::new(HashMap::new()),
            cursor_sinks: RefCell::new(HashMap::new()),
            applied_cursors: RefCell::new(HashMap::new()),
        }
    }

    /// Install the cursor backend. Called at build time by the engine
    /// builder after looking up the `Cursor` capability. The backend fires
    /// at runtime during `apply_changes` whenever the resolved cursor changes.
    pub fn set_cursor_platform(
        &mut self,
        backend: Arc<std::sync::Mutex<dyn crate::core::platform::CursorBackend + Send + Sync>>,
    ) {
        self.cursor_platform = Some(backend);
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
    /// [`PluginContext::clock`](crate::core::plugin::PluginContext::clock)
    /// and stash it in time-driven subsystems (animation, audio, …) so they
    /// can query `clock.now()` during their tick.
    pub fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }

    /// Record the latest pointer position for one view root (canvas-local
    /// logical pixels). Called by the event layer on routed `PointerMove`s.
    pub fn set_pointer_position(&mut self, root: ViewRootId, position: Offset) {
        self.pointer_positions.borrow_mut().insert(root, position);
    }

    /// Flush the cursor claims accumulated during paint of one root:
    /// resolve the deepest-wins value, dedup against the last applied cursor
    /// for that root, and on change invoke the installed `CursorBackend`'s
    /// `set_cursor`. Called by the driver once per root after its paint
    /// pass. `take` empties the sink so the next frame starts clean.
    pub fn apply_changes_for(&mut self, root: ViewRootId) {
        let Some(sink) = self.cursor_sinks.borrow().get(&root).cloned() else {
            return;
        };
        let resolved = sink.take().unwrap_or_default();
        let present = self.pointer_positions.borrow().contains_key(&root);
        if present && self.applied_cursors.borrow().get(&root) != Some(&resolved) {
            self.applied_cursors.borrow_mut().insert(root, resolved);
            #[allow(clippy::collapsible_if)]
            if let Some(backend) = self.cursor_platform.as_ref() {
                if let Ok(mut b) = backend.lock() {
                    b.set_cursor(resolved);
                }
            }
        }
    }

    /// Flush every root's cursor claims (in registration-agnostic map
    /// order). Called after the full per-root paint pass.
    pub fn apply_changes(&mut self) {
        let roots: Vec<ViewRootId> = self.cursor_sinks.borrow().keys().copied().collect();
        for root in roots {
            self.apply_changes_for(root);
        }
    }

    /// The most recent cursor applied for `root` (or `None` if no pointer
    /// position was ever recorded for it). Shipped to main via
    /// `MainMsg::CursorChanged` per root.
    pub fn last_applied_cursor_for(&self, root: ViewRootId) -> Option<Cursor> {
        self.applied_cursors.borrow().get(&root).copied()
    }

    /// Borrow the biz/paint face for one root's paint pass.
    pub fn paint_face_for(&self, root: ViewRootId) -> PaintShell<'_> {
        // Ensure a sink exists for this root before handing out the face.
        self.cursor_sinks.borrow_mut().entry(root).or_default();
        PaintShell { inner: self, root }
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
    root: ViewRootId,
}

impl<'a> PaintShell<'a> {
    /// Claim the host cursor for this frame. May be called many times during
    /// one paint pass (deepest painted region wins). Nothing is committed to
    /// the host until the driver flushes (`Shell::apply_changes_for`).
    pub fn set_cursor(&self, cursor: Cursor) {
        if let Some(sink) = self.inner.cursor_sinks.borrow().get(&self.root) {
            sink.set(cursor);
        }
    }

    /// Current frame time as a `Duration` since the epoch.
    pub fn now(&self) -> Duration {
        self.inner.now()
    }

    /// Last known pointer position in this root's local space, or `None` if
    /// no pointer move was received for this root.
    pub fn pointer_position(&self) -> Option<Offset> {
        self.inner
            .pointer_positions
            .borrow()
            .get(&self.root)
            .copied()
    }
}
