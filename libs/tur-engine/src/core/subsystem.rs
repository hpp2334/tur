//! Engine flush subsystems — long-lived, plugin-registered participants in
//! the per-frame [`TurAppInternal::flush`](crate::core::app::TurAppInternal::flush) loop.
//!
//! A [`Subsystem`] is anything that needs to advance its own time-driven state
//! once per frame (e.g. an animation manager that ticks its controllers). The
//! engine owns the registry; plugins register instances via
//! [`PluginContext::register_subsystem`](crate::core::plugin::PluginContext::register_subsystem)
//! during `build()`.
//!
//! ## Ordering
//!
//! Subsystems run in **registration order** — i.e. in the order plugins are
//! added to [`TurEngineBuilder`](crate::TurEngineBuilder). A subsystem that
//! must observe effects of an earlier subsystem's tick (within the same
//! `flush` iteration) should be registered after it.
//!
//! ## Frequency
//!
//! Each subsystem's [`flush`](Subsystem::flush) is called **once per
//! `TurAppInternal::flush` call** (= once per frame), not once per fixed-point
//! iteration. The engine gates internally so the tick happens at most once
//! even when the flush loop iterates multiple times to reach quiescence.

use boa_engine::Context;

/// A long-lived participant in the engine's per-frame flush loop.
///
/// Implementations advance their own time-driven state (e.g. animation
/// controllers, audio buffers) and report whether the engine should continue
/// iterating the fixed-point loop and/or schedule the next vsync frame.
///
/// See the [module docs](crate::core::subsystem) for ordering and frequency
/// guarantees.
pub trait Subsystem {
    /// Advance this subsystem's state by one frame. Called once per
    /// [`TurAppInternal::flush`](crate::core::app::TurAppInternal::flush) —
    /// not once per fixed-point iteration.
    ///
    /// Implementations should:
    ///   - query time via their own `Rc<dyn Clock>` (obtained at registration),
    ///   - mutate their own state,
    ///   - return an outcome describing whether the engine should continue
    ///     iterating the flush loop / schedule the next frame.
    fn flush(&mut self, cx: &mut SubsystemFlushContext<'_>) -> SubsystemOutcome;
}

/// Per-flush context passed to [`Subsystem::flush`]. Extensible — future
/// engine-provided state lands here without breaking the trait signature.
pub struct SubsystemFlushContext<'a> {
    /// The engine's boa `Context`. Borrowed for the duration of one subsystem
    /// tick; the borrow is released before the next subsystem (or the rest of
    /// the flush loop) runs.
    pub boa: &'a mut Context,
}

/// Outcome reported by a [`Subsystem`] after a flush tick.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SubsystemOutcome {
    /// This subsystem changed state in a way that requires another iteration
    /// of the flush fixed-point loop (e.g. enqueued reactive mutations that
    /// need to be drained). When any subsystem reports `dirtied = true`, the
    /// engine treats the frame as dirty and re-arms `need_paint`.
    pub dirtied: bool,
    /// This subsystem wants the engine to schedule the next vsync frame
    /// (e.g. an animation is still running). When any subsystem reports
    /// `request_frame = true`, the engine schedules
    /// [`NextFrame::Vsync`](crate::core::app::NextFrame::Vsync).
    pub request_frame: bool,
}

impl SubsystemOutcome {
    #[inline]
    #[must_use]
    pub fn idle() -> Self {
        Self::default()
    }

    #[inline]
    #[must_use]
    pub fn from_active(active: bool) -> Self {
        Self {
            dirtied: active,
            request_frame: active,
        }
    }
}
