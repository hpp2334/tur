//! Engine flush subsystems — long-lived, plugin-registered participants in
//! the per-frame [`TurAppInternal::flush`](crate::core::app::TurAppInternal::flush) loop.
//!
//! A [`Subsystem`] is anything that needs to either (a) advance its own
//! time-driven state once per frame (e.g. an animation manager that ticks its
//! controllers), or (b) react to drained platform/app events during the
//! engine's fixed-point flush loop (e.g. a scroll subsystem that processes
//! wheel events, a keyboard subsystem that routes key events to the focused
//! element). The engine owns the registry; plugins register instances via
//! [`PluginContext::register_subsystem`](crate::core::plugin::PluginContext::register_subsystem)
//! during `build()`.
//!
//! ## Ordering
//!
//! Subsystems run in **registration order** — i.e. the order plugins are
//! added to [`TurEngineBuilder`](crate::TurEngineBuilder). A subsystem that
//! must observe effects of an earlier subsystem's tick or event handler
//! (within the same `flush` iteration) should be registered after it.
//!
//! ## Frequency
//!
//! - [`Subsystem::flush`](Subsystem::flush) is called **once per
//!   `TurAppInternal::flush` call** (= once per frame), not once per
//!   fixed-point iteration. The engine gates internally so the tick happens
//!   at most once even when the flush loop iterates multiple times to reach
//!   quiescence.
//! - [`Subsystem::handle_platform_event`] and [`Subsystem::handle_app_event`]
//!   are called **per drained event**, every fixed-point iteration, in
//!   registration order. This matches what `AppHandler` did before the
//!   unification — each event is fanned out to every subsystem that cares
//!   about it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::Context;

use crate::core::async_::AsyncExecutor;
use crate::core::capability::Capabilities;
use crate::core::elements::NodeTree;
use crate::core::app::{AppEvent, AppEventQueue};
use crate::core::platform::{PlatformEvent, PlatformEventQueue};
use crate::core::focus::FocusManager;
use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::render::Renderer;
use crate::core::screen::Screen;

/// A long-lived participant in the engine's per-frame flush loop.
///
/// Implementations either advance their own time-driven state (e.g. animation
/// controllers, audio buffers — override [`flush`](Self::flush)) or react to
/// drained platform/app events (e.g. wheel/scroll, keyboard/IME — override
/// [`handle_platform_event`](Self::handle_platform_event) /
/// [`handle_app_event`](Self::handle_app_event)). All three methods default to
/// no-ops, so a subsystem overrides only the kind it cares about.
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
    fn flush(&mut self, _cx: &mut SubsystemFlushContext<'_>) -> SubsystemOutcome {
        SubsystemOutcome::idle()
    }

    /// React to a platform (input) event drained from
    /// [`PlatformEventQueue`](crate::core::platform::PlatformEventQueue).
    /// Invoked once per drained event, every fixed-point iteration, in
    /// registration order across all registered subsystems. Default: no-op.
    fn handle_platform_event(
        &mut self,
        _cx: &mut SubsystemFlushContext<'_>,
        _event: &PlatformEvent,
    ) {
    }

    /// React to an engine-internal event drained from
    /// [`AppEventQueue`](crate::core::app::AppEventQueue). Invoked
    /// once per drained event, every fixed-point iteration, in registration
    /// order across all registered subsystems. Default: no-op.
    fn handle_app_event(&mut self, _cx: &mut SubsystemFlushContext<'_>, _event: &AppEvent) {}
}

/// Per-flush context passed to every [`Subsystem`] method. The same shape is
/// used for the once-per-frame [`Subsystem::flush`] tick and the per-event
/// [`Subsystem::handle_platform_event`] / [`Subsystem::handle_app_event`]
/// dispatch.
///
/// The element tree / focus manager / mutation queue are exposed as the
/// shared `Rc<RefCell<...>>` handles (rather than pre-borrowed `&mut`s) so
/// subsystems that already hold their own Rc clone (e.g. `AnimationSubsystem`
/// captures the mutation queue at registration so controllers can enqueue
/// `onTick` mutations directly) don't panic on a double-borrow. Subsystems
/// borrow on demand via `cx.element_tree.borrow_mut()` etc.
///
/// Subsystems that just override [`Subsystem::flush`] (e.g. animation) only
/// need [`Self::boa`]; subsystems that handle events use the element tree /
/// focus manager / mutation queue / event queues / renderer /
/// `screen` / async executor / capability fields.
pub struct SubsystemFlushContext<'a> {
    /// The engine's boa `Context`. Borrowed for the duration of one subsystem
    /// tick or event dispatch; the borrow is released before the next
    /// subsystem (or the rest of the flush loop) runs.
    pub boa: &'a mut Context,
    /// Element tree (shared handle). Borrow on demand via `.borrow()` /
    /// `.borrow_mut()`.
    pub element_tree: NodeTree,
    pub focus_manager: Rc<RefCell<FocusManager>>,
    pub mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub platform_event_queue: &'a mut PlatformEventQueue,
    pub app_event_queue: &'a mut AppEventQueue,
    pub renderer: &'a mut dyn Renderer,
    /// Engine screen state — the canvas logical size + the `viewportSize$`
    /// atom. Driven by [`crate::core::screen::ResizeSubsystem`] on
    /// `PlatformEvent::Resize` (it sets the size, pushes the atom via
    /// [`Screen::sync_source`], and requests a paint). Other subsystems may
    /// read [`Screen::logical_size`]. Backed by `TurAppContext.screen`.
    pub screen: &'a mut Screen,
    pub need_paint: &'a Cell<bool>,
    /// Engine-owned async executor. Subsystems call `spawn_detached(...)` to
    /// run Rust futures (e.g. `clipboard.write_text`); the executor is driven
    /// each frame inside `flush`. See [`AsyncExecutor::spawn_detached`].
    pub async_executor: &'a Rc<AsyncExecutor>,
    /// Capability registry view. Subsystems look up plugin-injected backends
    /// (e.g. `Clipboard`, `Http`) at dispatch time via
    /// `cx.capabilities.of::<C>()`. Missing capabilities return `None` —
    /// subsystems must handle absence gracefully (typically silent drop with
    /// a `tracing::warn!`).
    pub capabilities: &'a Capabilities,
}

impl<'a> SubsystemFlushContext<'a> {
    /// Mark the current frame as needing a paint. Cheap — just flips a
    /// `Cell<bool>` the engine reads after the flush tick.
    pub fn request_paint(&self) {
        self.need_paint.set(true);
    }
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
