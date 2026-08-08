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
//! added to [`TurRuntimeBuilder`](crate::TurRuntimeBuilder). A subsystem that
//! must observe effects of an earlier subsystem's tick or event handler
//! (within the same `flush` iteration) should be registered after it.
//!
//! Within one fixed-point iteration there are two flush phases, straddling
//! the layout step:
//!
//! 1. [`Subsystem::flush_pre_layout`] — runs **before** layout. Used for
//!    time-driven state advance (e.g. an animation manager that ticks its
//!    controllers and enqueues mutations drained later in the iteration).
//! 2. **layout** — the engine lays out dirty nodes.
//! 3. [`Subsystem::flush_post_layout`] — runs **after** layout. Used for
//!    layout-derived recomputation that must read fresh geometry (e.g. the
//!    `CompositedTransformSubsystem` mapping a target's world position onto
//!    its follower). Without this post-layout phase, a follower would read
//!    zero/stale sizes on the first frame and only self-correct on the next
//!    input event.
//!
//! ## Frequency
//!
//! - Both flush phases are called **once per fixed-point iteration** of
//!   `TurAppInternal::flush` (i.e. possibly several times per frame, once per
//!   iteration until the loop reaches quiescence). A subsystem that advances
//!   time-driven state (e.g. an animation manager sampling the clock) must
//!   self-gate so it advances **at most once per frame** — use
//!   [`SubsystemFlushContext::frame_id`], a per-`flush()` epoch that is stable
//!   across iterations within one frame but differs across frames, and record
//!   the last id it advanced for. Signals (`mark_dirty` / `request_paint` /
//!   `request_next_frame`) are cheap and idempotent, so calling them every
//!   iteration is fine.
//! - [`Subsystem::handle_platform_event`] and [`Subsystem::handle_app_event`]
//!   are called **per drained event**, every fixed-point iteration, in
//!   registration order. This matches what `AppHandler` did before the
//!   unification — each event is fanned out to every subsystem that cares
//!   about it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::Context;

use crate::core::app::{AppEvent, AppEventQueue};
use crate::core::capability::Capabilities;
use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::elements::NodeTree;
use crate::core::focus::FocusManager;
use crate::core::platform::{PlatformEvent, PlatformEventQueue};
use crate::core::screen::Screen;

/// A long-lived participant in the engine's per-frame flush loop.
///
/// Implementations either advance their own time-driven state (e.g. animation
/// controllers, audio buffers — override
/// [`flush_pre_layout`](Self::flush_pre_layout)) or react to drained
/// platform/app events (e.g. wheel/scroll, keyboard/IME — override
/// [`handle_platform_event`](Self::handle_platform_event) /
/// [`handle_app_event`](Self::handle_app_event)). Layout-derived
/// recomputation that needs fresh post-layout geometry goes in
/// [`flush_post_layout`](Self::flush_post_layout). All four methods default to
/// no-ops, so a subsystem overrides only the kind it cares about.
///
/// Neither flush phase returns anything — subsystems push intent back into the
/// engine via the context:
/// [`SubsystemFlushContext::mark_dirty`](crate::core::subsystem::SubsystemFlushContext::mark_dirty)
/// (re-layout + paint this frame),
/// [`SubsystemFlushContext::request_paint`](crate::core::subsystem::SubsystemFlushContext::request_paint)
/// (paint this frame), and
/// [`SubsystemFlushContext::request_next_frame`](crate::core::subsystem::SubsystemFlushContext::request_next_frame)
/// (schedule the next vsync). See the [module docs](crate::core::subsystem)
/// for ordering and frequency guarantees.
pub trait Subsystem {
    /// Advance this subsystem's state. Called **every fixed-point iteration**
    /// of `TurAppInternal::flush` (possibly several times per frame), in
    /// registration order across subsystems, and **before** the layout step
    /// of that iteration.
    ///
    /// Implementations should:
    ///   - gate time-driven work via [`SubsystemFlushContext::frame_id`] so
    ///     the clock/state advances at most once per frame (a frame spans many
    ///     iterations),
    ///   - mutate their own state,
    ///   - signal the engine via [`SubsystemFlushContext::mark_dirty`] /
    ///     [`SubsystemFlushContext::request_paint`] /
    ///     [`SubsystemFlushContext::request_next_frame`].
    fn flush_pre_layout(&mut self, _cx: &mut SubsystemFlushContext<'_>) {}

    /// Recompute layout-derived state. Called **every fixed-point iteration**,
    /// in registration order across subsystems, **after** the layout step of
    /// that iteration — so `computed_layout` and
    /// [`crate::core::elements::NodeTreeData::absolute_affine_of`] reflect the
    /// freshly-laid-out tree. Use this for anything that must read final
    /// geometry (e.g. `CompositedTransformSubsystem` mapping a target's world
    /// position onto its follower). Default: no-op.
    ///
    /// Signalling is the same as for [`Self::flush_pre_layout`]. Writing
    /// paint-only state (e.g. a link-tracked transform read via
    /// [`crate::core::render::ElementRender::relative_transform`]) does not
    /// require another layout pass; the engine paints with whatever this phase
    /// last wrote.
    fn flush_post_layout(&mut self, _cx: &mut SubsystemFlushContext<'_>) {}

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

/// Per-`TurAppInternal::flush` signalling channels the engine exposes to
/// subsystems, bundled so they can be threaded through the per-iteration
/// `flush` tick and the per-event `dispatch_*` paths as a single reference.
///
/// Built once at the top of each `flush()` call; the same reference is shared
/// with every [`SubsystemFlushContext`] constructed during that call.
/// `frame_id` is a per-`flush()` epoch (stable across iterations, differs
/// across calls); `sub_dirty` / `sub_request_frame` are the accumulators
/// behind [`SubsystemFlushContext::mark_dirty`] /
/// [`SubsystemFlushContext::request_next_frame`].
pub struct FlushSignals<'a> {
    pub frame_id: u64,
    pub sub_dirty: &'a Cell<bool>,
    pub sub_request_frame: &'a Cell<bool>,
}

/// Per-flush context passed to every [`Subsystem`] method. The same shape is
/// used for the per-iteration [`Subsystem::flush`] tick and the per-event
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
/// Subsystems that just override [`Subsystem::flush_pre_layout`] (e.g.
/// animation) only need [`Self::boa`]; subsystems that handle events use the
/// element tree / focus manager / mutation queue / event queues /
/// `screen` / async executor / capability fields.
///
/// ## Signalling the engine
///
/// Instead of returning an outcome, a `flush` (or event handler) pushes
/// intent into the engine via:
///   - [`Self::mark_dirty`] — the subsystem changed layout-affecting state;
///     the engine re-lays-out and marks the frame for paint this iteration.
///   - [`Self::request_paint`] — paint this frame (no re-layout necessarily).
///   - [`Self::request_next_frame`] — schedule the next vsync (e.g. an
///     animation is still running). This is the signal that keeps time-driven
///     work advancing frame-to-frame; it accumulates across all iterations of
///     a single `flush()` and feeds the post-loop schedule decision.
///
/// `frame_id` lets a subsystem self-gate "advance once per frame" work (clock
/// sampling): it is stable across the fixed-point iterations of one
/// `TurAppInternal::flush` call and differs across `flush` calls.
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
    /// Engine screen state — the canvas logical size + the `viewportSize$`
    /// atom. Driven by [`crate::core::screen::ResizeSubsystem`] on
    /// `PlatformEvent::Resize` (it sets the size, pushes the atom via
    /// [`Screen::sync_source`], and requests a paint). Other subsystems may
    /// read [`Screen::logical_size`]. Backed by `TurAppContext.screen`.
    pub screen: &'a mut Screen,
    pub need_paint: &'a Cell<bool>,
    /// Worker-thread scheduler. Subsystems call
    /// [`WorkerScheduler::spawn_local`] to drive Rust futures (e.g.
    /// `clipboard.write_text`). The future's completion pushes a closure
    /// via [`Self::completion_handle`]; the engine drains it on the next
    /// flush iteration.
    pub worker_sched: &'a crate::core::scheduler::WorkerScheduler,
    /// Completion handle for spawned futures. A spawned future calls
    /// `completion_handle.push(closure)` from inside its body to settle a
    /// `JsPromise` (or similar) under `&mut Context` on the next flush.
    /// Pushing fires `on_push`, which self-sends `WorkerMsg::Wake` so the
    /// worker flushes promptly.
    pub completion_handle: &'a crate::core::async_::CompletionHandle,
    /// Capability registry view. Subsystems look up plugin-injected backends
    /// (e.g. `Clipboard`, `Http`) at dispatch time via
    /// `cx.capabilities.of::<C>()`. Missing capabilities return `None` —
    /// subsystems must handle absence gracefully (typically silent drop with
    /// a `tracing::warn!`).
    pub capabilities: &'a Capabilities,
    /// Per-`flush()` epoch. Stable across the fixed-point iterations of one
    /// `TurAppInternal::flush` call; differs across `flush` calls. A
    /// subsystem that samples the clock should advance at most once per frame
    /// by recording the last `frame_id` it advanced for.
    pub frame_id: u64,
    /// Accumulator for [`Self::mark_dirty`]: any subsystem that flips it
    /// forces the engine to re-lay-out (and marks the frame for paint) this
    /// iteration. Owned by the flush loop; `.take()`n after each iteration.
    pub sub_dirty: &'a Cell<bool>,
    /// Accumulator for [`Self::request_next_frame`]: any subsystem that flips
    /// it makes the engine schedule the next vsync. Owned by the flush loop;
    /// read once after the loop to decide the next-frame schedule.
    pub sub_request_frame: &'a Cell<bool>,
}

impl<'a> SubsystemFlushContext<'a> {
    /// Mark the current frame as needing a paint. Cheap — just flips a
    /// `Cell<bool>` the engine reads after the flush tick.
    pub fn request_paint(&self) {
        self.need_paint.set(true);
    }

    /// The subsystem changed state that requires another layout pass this
    /// iteration (and a paint). The engine folds this into its per-iteration
    /// dirty decision. Does NOT by itself keep the fixed-point loop iterating
    /// — loop continuation is driven by pending mutations / events / reactive
    /// changes, which a ticking subsystem typically enqueues.
    pub fn mark_dirty(&self) {
        self.sub_dirty.set(true);
    }

    /// Request that the engine schedule the next vsync frame (e.g. because an
    /// animation is still running). Accumulates across all iterations of a
    /// single `flush()` call and feeds the post-loop schedule decision
    /// (`NextFrame::Vsync`). Cheap and idempotent — safe to call every
    /// iteration.
    pub fn request_next_frame(&self) {
        self.sub_request_frame.set(true);
    }

    /// Per-`flush()` epoch — see [`SubsystemFlushContext::frame_id`].
    #[inline]
    #[must_use]
    pub fn frame_id(&self) -> u64 {
        self.frame_id
    }
}
