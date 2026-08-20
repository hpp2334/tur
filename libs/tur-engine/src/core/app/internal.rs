use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::context::time::Clock;

use crate::core::app::TurAppContext;
use crate::core::async_::{CompletionHandle, CompletionQueue, FlushTaskQueue, TurJobExecutor};
use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::js_runtime::TurInstanceContext;
use crate::core::render::RenderCommand;
use crate::core::scheduler::WorkerContext;
use crate::core::subsystem::Subsystem;

use crate::core::fonts::{FontContext, FontLoader};
use crate::error::TurError;

/// Engine → embedder: how to schedule the next frame after a `flush`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextFrame {
    /// Nothing time-driven is pending — the loop can go idle until the next
    /// platform input arrives.
    Idle,
    /// A continuous animation is running — re-arm on the next vsync (i.e.
    /// request another animation frame immediately).
    Vsync,
}

/// Outcome of a single [`TurAppInternal::flush`] / `pump` call.
#[derive(Debug, Clone, Copy)]
pub struct FrameOutcome {
    /// Whether a new frame was actually painted this call.
    pub painted: bool,
    /// How the caller should schedule the next frame.
    pub schedule: NextFrame,
}

pub struct TurAppInternal {
    pub(crate) js_context: TurInstanceContext,
    pub(crate) app_context: Rc<RefCell<TurAppContext>>,
    pub(crate) executor: Rc<TurJobExecutor>,
    /// Worker-thread scheduler. Bridges grab it via
    /// [`PluginContext::worker_ctx`] / [`SubsystemFlushContext::worker_ctx`]
    /// and call `spawn_local(fut)` to drive async work (clipboard reads,
    /// http requests, sleep futures). The driver's `sleep` returns a
    /// platform-specific `Sleep(BoxFuture)`.
    #[allow(dead_code)]
    pub(crate) worker_ctx: WorkerContext,
    /// Completion queue — closures pushed by spawned futures (e.g. promise
    /// settle closures) are drained inside `flush()` under `&mut Context`.
    /// The `on_push` callback self-sends `WorkerMsg::Wake` to ensure the
    /// worker flushes promptly whenever a future completes.
    pub(crate) completion_queue: Rc<CompletionQueue>,
    /// Cheap-cloned handle on the completion queue, handed out to bridges
    /// via [`PluginContext::completion_handle`] /
    /// [`SubsystemFlushContext::completion_handle`].
    #[allow(dead_code)]
    pub(crate) completion_handle: CompletionHandle,
    /// Flush-driven task queue for engine-internal async (`sleep`,
    /// `launch`). Tasks pushed here are polled every fixed-point iteration
    /// of `flush()` so a sleep whose deadline is reached by a clock
    /// advance resolves *inside* the same flush (instead of lagging to the
    /// next frame, which the single-pump-per-tick countdown tests never
    /// observe). Real platform async (HTTP / clipboard / file-picker)
    /// still uses `worker_ctx.spawn_local`.
    pub(crate) flush_task_queue: Rc<FlushTaskQueue>,
    /// Plugin-registered flush subsystems. Each is `flush`-ed **every
    /// fixed-point iteration** of `flush()` (possibly several times per
    /// frame), in registration order, before `flush_reactive`. Time-driven
    /// subsystems self-gate via the per-`flush()` `frame_id` so the clock
    /// advances at most once per frame. The same `Rc<RefCell<…>>` is shared
    /// with [`PluginContext`](crate::core::plugin::PluginContext) so plugins
    /// can push into the vec during `register`.
    pub(crate) subsystems: Rc<RefCell<Vec<Box<dyn Subsystem>>>>,
    /// Per-`flush()` epoch exposed to subsystems via
    /// [`crate::core::subsystem::SubsystemFlushContext::frame_id`].
    /// Incremented once at the top of each `flush()` call; stable across the
    /// fixed-point iterations within that call.
    pub(crate) frame_id: Cell<u64>,
    /// Always-installed event bus — bidirectional byte channel
    /// between the Rust host and the JS realm. Created in
    /// [`TurAppInternal::new`]; the host-side handle is retrieved via
    /// [`crate::TurApp::event_bus`]. Plugins (specifically
    /// `install_event_bus`) read this via
    /// [`crate::core::plugin::PluginContext::event_bus`] to register the
    /// JS-side bridge (`eventBus.on`/`send`) and the
    /// [`EmbedderBusSubsystem`] that drains the queues
    /// each flush.
    ///
    /// [`EmbedderBusSubsystem`]: crate::core::event_bus::EmbedderBusSubsystem
    pub(crate) event_bus: Rc<crate::core::event_bus::EventBus>,
    /// Worker → main render-command batch produced by the last `flush()`
    /// that painted. Drained by `AppBackend`'s `worker_loop` and shipped
    /// to main via `HostMsg::RenderCommands`. `None` if no paint happened
    /// this flush (or already drained).
    pub(crate) pending_render_batch: RefCell<Option<Vec<RenderCommand>>>,
}

/// RAII guard set up at `flush()` entry; clears `in_flush` on drop so the
/// worker is "idle" again for out-of-flush self-wakes. Drop runs on every
/// exit path (normal return or future `?`), guaranteeing `end_flush` pairs
/// with `begin_flush`.
struct FlushGuard<'a>(&'a TurAppInternal);

impl Drop for FlushGuard<'_> {
    fn drop(&mut self) {
        self.0.js_context.end_flush();
    }
}

impl TurAppInternal {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        font_context: FontContext,
        font_loader: std::sync::Arc<dyn FontLoader>,
        executor: Rc<TurJobExecutor>,
        clock: std::sync::Arc<dyn Clock + Send + Sync>,
        capabilities: crate::core::capability::Capabilities,
        worker_ctx: WorkerContext,
        wake_worker: std::sync::Arc<dyn Fn() + Send + Sync>,
        host_tx: crate::core::app::HostTx,
    ) -> Self {
        use crate::core::edgy::mutation::PendingMutationInvocationQueue;
        use crate::core::edgy::reactive::Store;
        use crate::core::elements::NodeTree;
        use crate::core::focus::FocusManager;
        use crate::core::image_resource::ImageManager;

        let mutation_queue = Rc::new(RefCell::new(PendingMutationInvocationQueue::new()));
        let focus_manager = Rc::new(RefCell::new(FocusManager::new()));
        let dirty = Rc::new(Cell::new(false));
        let need_paint = Rc::new(Cell::new(false));

        // Event bus — created early so `host_tx` can be set before any
        // flush runs. The `EmbedderBusSubsystem` ships JS→host bytes to main
        // via this sender; without it, host-side `on_bus_event` handlers
        // never fire.
        let event_bus = Rc::new(crate::core::event_bus::EventBus::new());
        event_bus.set_host_tx(host_tx.clone());

        // Worker-side image state: metadata (sizes) + next-id counter,
        // bundled in one `ImageManager`. The pixel `Blob` ships to main
        // directly from the `createImageResource` bridge via the shared
        // `host_tx` channel (one `HostMsg::UploadImage` per decode). The
        // worker never retains pixels across a frame boundary.
        let image_manager = Rc::new(RefCell::new(ImageManager::new()));

        // Adapt the shared `Arc<dyn Clock + Send + Sync>` to the
        // `Rc<dyn Clock>` that `FrameEnv` expects (per-instance + worker-side
        // only, never shared across threads). ClockProxy is a Sized adapter
        // that delegates to the Arc.
        let clock_rc: Rc<dyn Clock> = Rc::new(crate::core::runtime::ClockProxy(clock));

        // Completion queue: closures pushed by spawned futures (e.g. promise
        // settle closures) are drained inside `flush()` under `&mut Context`.
        // The `on_push` callback self-sends `WorkerMsg::Wake` so the worker
        // flushes promptly whenever a future completes — without it, an
        // idle worker would never wake to drain a completion arriving
        // between frames. The same `wake_worker` Arc is shared with the
        // flush-driven task queue below (it doubles as the task waker,
        // which sleep futures register with the test `VirtualClock`).
        let completion_queue = Rc::new(CompletionQueue::new(wake_worker.clone()));
        let completion_handle = completion_queue.handle();
        // Flush-driven task queue: `sleep` + `launch` push their driver
        // futures here (instead of `worker_ctx.spawn_local`) so `flush`
        // polls them in lockstep with completions / microtasks — closing
        // the cross-frame lag that otherwise breaks single-frame sleep
        // semantics. See `async_::flush_tasks`.
        let flush_task_queue = Rc::new(FlushTaskQueue::new(wake_worker.clone()));

        let store = Store::new(dirty.clone());
        let element_tree = NodeTree::new(store.clone());

        let js_context = TurInstanceContext::new(
            element_tree.clone(),
            mutation_queue.clone(),
            focus_manager.clone(),
            dirty,
            need_paint,
            image_manager.clone(),
            host_tx,
            store.clone(),
            worker_ctx.clone(),
            completion_handle.clone(),
            flush_task_queue.handle(),
            wake_worker.clone(),
            capabilities,
        );

        // Share the capability registry between the JS context (bridge fns)
        // and the app context (subsystems via SubsystemFlushContext). Both hold the
        // same `Rc<RefCell<HashMap>>` via the `Capabilities` view clone.
        let capabilities = js_context.capability();

        let app_context = TurAppContext::new(
            element_tree,
            mutation_queue,
            focus_manager,
            image_manager,
            font_context,
            font_loader,
            worker_ctx.clone(),
            completion_handle.clone(),
            capabilities,
            clock_rc,
            store,
        );

        Self {
            js_context,
            app_context: Rc::new(RefCell::new(app_context)),
            executor,
            worker_ctx,
            completion_queue,
            completion_handle,
            flush_task_queue,
            subsystems: Rc::new(RefCell::new(Vec::new())),
            frame_id: Cell::new(0),
            event_bus: event_bus.clone(),
            pending_render_batch: RefCell::new(None),
        }
    }

    pub fn flush(&self, boa_context: &mut boa_engine::Context) -> Result<FrameOutcome, TurError> {
        // Enter the flush window: mark in-flush (so out-of-flush self-wakes
        // raised by `request_paint` / `set_dirty` during this flush don't
        // emit redundant `Wake`s) and re-arm the wake coalescing gate for
        // any paint request raised mid-flush (it must emit a fresh wake for
        // the *next* pump). See `TurInstanceContext::begin_flush` / `end_flush`.
        self.js_context.begin_flush();
        let _flush_guard = FlushGuard(self);
        let mut needs_paint = false;
        // Per-`flush()` epoch, bumped once per call. Stable across the
        // fixed-point iterations below so subsystems can self-gate "advance
        // once per frame" (clock sampling) via `cx.frame_id()`.
        let frame_id = {
            let next = self.frame_id.get().wrapping_add(1);
            self.frame_id.set(next);
            next
        };
        // Per-iteration dirty flag (subsystems flip via `cx.mark_dirty`) and
        // per-`flush()` schedule accumulator (subsystems flip via
        // `cx.request_frame`). `sub_dirty` is taken after each iteration
        // and folded into the per-iteration dirty decision; `sub_request_frame`
        // accumulates across all iterations and feeds the post-loop schedule.
        let sub_dirty = Cell::new(false);
        let sub_request_frame = Cell::new(false);
        // Bundled channels shared with every subsystem context this `flush()`.
        let signals = crate::core::subsystem::FlushSignals {
            frame_id,
            sub_dirty: &sub_dirty,
            sub_request_frame: &sub_request_frame,
        };

        loop {
            // Drain completions produced by spawned futures since the last
            // flush iteration. Completions settle JsPromises (e.g.
            // clipboard read resolve) under `&mut Context`, enqueuing
            // PromiseJobs that boa's microtask drain (below) picks up.
            self.completion_queue.drain(boa_context);

            // Poll engine-internal async tasks (`sleep`, `launch`) pushed
            // to the flush-driven queue. Done BEFORE the rest of the
            // iteration so a sleep that just resolved pushes its
            // completion this same iteration (drained at the top of the
            // NEXT iteration) and the launch driver resumes in lockstep.
            // `tasks_completed > 0` keeps the fixed-point loop alive to
            // drain those completions. See `async_::flush_tasks`.
            let tasks_completed = self.flush_task_queue.poll_all();

            let handled_events = self.flush_app_events(boa_context, &signals);

            // Pre-layout subsystem flush — runs every fixed-point iteration,
            // in registration order, BEFORE the layout step. Each subsystem
            // owns its own clock + state; time-driven ones self-gate via
            // `cx.frame_id()` so the clock advances at most once per frame.
            // Subsystems push intent back via `cx.mark_dirty()` /
            // `cx.request_paint()` / `cx.request_frame()` instead of
            // returning an outcome.
            //
            // Animation (registered via `tur-animation::TurAnimationPlugin`)
            // is the canonical example: it ticks active
            // `AnimationController`s once per frame here (gated by frame_id),
            // enqueuing `onTick`/`onEnd` mutations that fire later in
            // `flush_pending_mutations`, and calls `request_frame()`
            // every iteration a controller is active — including iterations
            // where a controller was registered mid-frame (e.g. from an
            // event/lifecycle handler). That is what keeps an animation
            // started from a callback advancing without waiting for the next
            // platform input.
            let subsystem_dirtied = {
                let need_paint = self.js_context.need_paint.clone();
                let mut ctx_guard = self.app_context.borrow_mut();
                let ctx: &mut crate::core::app::TurAppContext = &mut ctx_guard;
                let mut cx = crate::core::subsystem::SubsystemFlushContext {
                    boa: boa_context,
                    element_tree: ctx.element_tree.clone(),
                    focus_manager: ctx.focus_manager.clone(),
                    mutation_queue: ctx.mutation_queue.clone(),
                    platform_event_queue: &mut ctx.platform_event_queue,
                    app_event_queue: &mut ctx.app_event_queue,
                    screen: &mut ctx.screen,
                    need_paint: &need_paint,
                    worker_ctx: &ctx.worker_ctx,
                    completion_handle: &ctx.completion_handle,
                    capabilities: &ctx.capabilities,
                    frame_id: signals.frame_id,
                    sub_dirty: signals.sub_dirty,
                    sub_request_frame: signals.sub_request_frame,
                };
                for sub in self.subsystems.borrow_mut().iter_mut() {
                    sub.flush_pre_layout(&mut cx);
                }
                // `cx` (and its `ctx_guard` borrow) drop here, before the
                // layout/render borrows below.
                drop(cx);
                drop(ctx_guard);
                sub_dirty.take()
            };

            // Reactive flush: drain the store, expand dirty atoms, and dispatch
            // `do_update(dirties)` to the mounted root. This may mutate
            // the ElementTree, which sets `dirty`/`need_paint` for the next
            // layout pass.
            let (reactive_changed, dirty_element_ids) = self.flush_reactive(boa_context, frame_id);

            // LazyList remount now happens *inside* `perform_layout` (it uses
            // the real viewport from constraints), so there is no separate
            // pre-layout remount pass here.
            let dirty = self.js_context.dirty.take()
                || self.js_context.need_paint.take()
                || reactive_changed
                || subsystem_dirtied;
            if dirty {
                needs_paint = true;
                self.app_context
                    .borrow_mut()
                    .layout(self.js_context.dirty.clone(), boa_context);
            }
            // Post-layout subsystem flush — runs every fixed-point iteration, in
            // registration order, AFTER the layout step, so subscribers read the
            // freshly-laid-out tree. This is where layout-derived recomputation
            // lives: e.g. `CompositedTransformSubsystem` maps each target's world
            // position onto its follower using final geometry + the follower's
            // just-resolved anchor cache. Without this phase a follower would read
            // zero/stale sizes on the first frame and only self-correct on the
            // next input event (see `follower_correct_on_first_frame_non_topleft_anchor`).
            {
                let need_paint = self.js_context.need_paint.clone();
                let mut ctx_guard = self.app_context.borrow_mut();
                let ctx: &mut crate::core::app::TurAppContext = &mut ctx_guard;
                let mut cx = crate::core::subsystem::SubsystemFlushContext {
                    boa: boa_context,
                    element_tree: ctx.element_tree.clone(),
                    focus_manager: ctx.focus_manager.clone(),
                    mutation_queue: ctx.mutation_queue.clone(),
                    platform_event_queue: &mut ctx.platform_event_queue,
                    app_event_queue: &mut ctx.app_event_queue,
                    screen: &mut ctx.screen,
                    need_paint: &need_paint,
                    worker_ctx: &ctx.worker_ctx,
                    completion_handle: &ctx.completion_handle,
                    capabilities: &ctx.capabilities,
                    frame_id: signals.frame_id,
                    sub_dirty: signals.sub_dirty,
                    sub_request_frame: signals.sub_request_frame,
                };
                for sub in self.subsystems.borrow_mut().iter_mut() {
                    sub.flush_post_layout(&mut cx);
                }
                // `cx` (and its `ctx_guard` borrow) drop here before the
                // lifecycle/render borrows below.
                drop(cx);
                drop(ctx_guard);
            }
            // Lifecycle hooks fire after layout: on_mounted for inserted
            // elements, on_updated for dirtied elements, before_destroy for
            // removed elements. Pushed mutations are drained right after.
            self.run_lifecycle_hooks(boa_context, &dirty_element_ids);
            {
                let mut cx = crate::core::view::SharedViewCx::new(self.js_context.clone());
                cx.flush_focus_notifications(boa_context);
            }
            let handled_mutations = self.flush_pending_mutations(boa_context);
            // Run boa microtasks (PromiseJobs, GenericJobs, AsyncJobs).
            // PromiseJobs fire `.then` callbacks which may call bridge fns
            // that spawn more Rust futures via `worker_ctx.spawn_local`.
            // Those futures' completions are drained at the top of the next
            // iteration, keeping the fixed-point loop alive.
            let jobs_run = self.executor.drain(boa_context).unwrap_or(0);
            let new_dirty = self.js_context.dirty.get() || self.js_context.need_paint.get();
            // Quiescence: no events, no mutations, no dirty state, no
            // completions drained this iteration, no microtasks ran, and no
            // flush-driven task completed (a completed task likely pushed a
            // completion we need to drain next iteration).
            if !handled_events
                && !handled_mutations
                && !new_dirty
                && jobs_run == 0
                && tasks_completed == 0
            {
                break;
            }
        }

        if needs_paint {
            // Record the paint pass into a `Vec<RenderCommand>`; main
            // applies it to its renderer (`AppBackend::render_batch`).
            let batch = self.app_context.borrow_mut().build_render_batch();
            *self.pending_render_batch.borrow_mut() = Some(batch);
        }

        // Decide how the caller should schedule the next frame.
        //
        // - `Vsync`: a subsystem requested a frame (e.g. an animation is
        //   running). Sleep-driven async work drives its own wake via
        //   `CompletionHandle::on_push` (self-sends Wake), so it doesn't
        //   keep the loop busy on idle.
        // - `Idle`: nothing time-driven is pending — the loop stops until
        //   the next platform input or async completion.
        let schedule = if sub_request_frame.get() {
            NextFrame::Vsync
        } else {
            NextFrame::Idle
        };

        Ok(FrameOutcome {
            painted: needs_paint,
            schedule,
        })
    }

    /// Drain the reactive store and mark affected tree nodes dirty via the
    /// subscriber graph. Returns `(reactive_changed, dirty_element_ids)`:
    /// the element ids whose subscribed atoms changed this flush — used by
    /// the flush loop to fire `on_updated` lifecycle hooks after layout.
    ///
    /// Also delivers `watch()` callbacks: due watchers (their watched atom is
    /// dirtied, at most once per `frame_id`) are pushed onto the mutation
    /// queue, so `flush_pending_mutations` invokes them later this iteration
    /// — same rail, same frame, against the mounted store.
    fn flush_reactive(
        &self,
        boa_context: &mut boa_engine::Context,
        frame_id: u64,
    ) -> (bool, Vec<ElementNodeId>) {
        let store = self.js_context.store.clone();
        let flush_engine = store.flush_engine();
        if !flush_engine.has_pending() {
            return (false, Vec::new());
        }
        let dirties = flush_engine.flush_atoms();
        if dirties.is_empty() {
            return (false, Vec::new());
        }

        // Watchers (non-element subscribers) — queue due callbacks before the
        // element work below; the mutation drain later this iteration invokes
        // them with the mounted store's ctx.
        let due_callbacks = store.watch_dispatch().due_callbacks(&dirties, frame_id);
        if !due_callbacks.is_empty() {
            let mut queue = self.js_context.mutation_queue.borrow_mut();
            for callback in due_callbacks {
                queue.push(
                    crate::core::edgy::mutation::MutationHandle::<()>::new(callback),
                    (),
                );
            }
        }

        let dirty_subs = store.subscriber_index().dirty_subscribers(&dirties);

        // Mark all dirty subscribers dirty. mark_dirty handles fragments by
        // skipping them and marking their real parent element.
        {
            let mut tree = self.js_context.element_tree.borrow_mut();
            for sub_id in &dirty_subs {
                tree.mark_dirty(NodeId::new(sub_id.as_u64()));
            }
        }

        // Split dirty subscribers into elements and fragments so fragment
        // rebuilds only process dirty fragments (not a full scan).
        let dirty_frag_ids: Vec<FragmentNodeId> = {
            let tree = self.js_context.element_tree.borrow();
            dirty_subs
                .iter()
                .filter(|s| tree.is_fragment(NodeId::new(s.as_u64())))
                .map(|s| FragmentNodeId::new(s.as_u64()))
                .collect()
        };
        // Element ids dirtied this flush (for the post-layout `on_updated` pass).
        let dirty_element_ids: Vec<ElementNodeId> = {
            let tree = self.js_context.element_tree.borrow();
            dirty_subs
                .iter()
                .filter(|s| !tree.is_fragment(NodeId::new(s.as_u64())))
                .map(|s| ElementNodeId::new(s.as_u64()))
                .collect()
        };

        // Fragment rebuilds (Condition / Each / Switch branch swaps).
        self.rebuild_fragments(boa_context, &dirty_frag_ids);

        (!dirty_subs.is_empty(), dirty_element_ids)
    }

    /// Fire element lifecycle hooks: `on_mounted` for newly-inserted elements,
    /// `on_updated` for elements whose subscribed atoms were dirtied this
    /// flush, and `before_destroy` for elements removed since the last pass.
    /// All hooks run after layout (so the mutation queue is drained by the
    /// subsequent `flush_pending_mutations`).
    fn run_lifecycle_hooks(
        &self,
        boa_context: &mut boa_engine::Context,
        dirty_element_ids: &[ElementNodeId],
    ) {
        let mut cx = crate::core::view::SharedViewCx::new(self.js_context.clone());

        // on_mounted — freshly-inserted elements.
        let mounted_ids = self
            .js_context
            .element_tree
            .borrow_mut()
            .take_pending_mounted();
        for id in mounted_ids {
            let mut element = {
                let mut tree = self.js_context.element_tree.borrow_mut();
                tree.get_element_mut(id).and_then(|n| n.element.take())
            };
            if let Some(ref mut elem) = element {
                elem.run_on_mounted(&mut cx, boa_context);
            }
            if let Some(elem) = element {
                let mut tree = self.js_context.element_tree.borrow_mut();
                if let Some(node) = tree.get_element_mut(id) {
                    node.element = Some(elem);
                }
            }
        }

        // on_updated — elements dirtied this flush (post-layout).
        for id in dirty_element_ids {
            let mut element = {
                let mut tree = self.js_context.element_tree.borrow_mut();
                tree.get_element_mut(*id).and_then(|n| n.element.take())
            };
            if let Some(ref mut elem) = element {
                elem.run_on_updated(&mut cx, boa_context);
            }
            if let Some(elem) = element {
                let mut tree = self.js_context.element_tree.borrow_mut();
                if let Some(node) = tree.get_element_mut(*id) {
                    node.element = Some(elem);
                }
            }
        }

        // before_destroy — elements removed since the last pass. The element
        // is already detached from the tree (taken out during destroy), so we
        // just fire the hook and let it drop.
        let destroyed = self
            .js_context
            .element_tree
            .borrow_mut()
            .take_pending_destroy();
        for mut elem in destroyed {
            elem.run_before_destroy(&mut cx, boa_context);
        }
    }

    /// Rebuild dirty fragments (Condition / Each / Switch). Only fragments
    /// whose subscribed atoms are dirty are processed — identified via the
    /// subscriber graph, not a full scan. Each fragment's `perform_update`
    /// resolves the current value and swaps the branch/items if changed.
    fn rebuild_fragments(
        &self,
        boa_context: &mut boa_engine::Context,
        dirty_frag_ids: &[FragmentNodeId],
    ) {
        let mut cx = crate::core::view::SharedViewCx::new(self.js_context.clone());

        for fid in dirty_frag_ids {
            let mut kind = {
                let mut tree = self.js_context.element_tree.borrow_mut();
                tree.get_fragment_mut(*fid).and_then(|h| h.kind.take())
            };
            let Some(ref mut k) = kind else { continue };

            // Save old children + parent BEFORE rebuild (perform_update
            // auto-links new children to frag.children via append_child).
            let (old_children, parent) = {
                let tree = self.js_context.element_tree.borrow();
                tree.get_fragment(*fid)
                    .map(|f| (f.children.clone(), f.parent))
                    .unwrap_or((Vec::new(), (*fid).into()))
            };

            let new_children = k.perform_update(&mut cx, boa_context, *fid);

            if let Some(new) = new_children {
                // frag.children now has old + new; replace with just new.
                {
                    let mut tree = self.js_context.element_tree.borrow_mut();
                    if let Some(f) = tree.get_fragment_mut(*fid) {
                        f.children = new;
                    }
                }
                // Destroy old subtrees.
                for child in &old_children {
                    cx.destroy_child(*child);
                }
                cx.mark_dirty(parent);
            }

            // Put kind back.
            if let Some(kind) = kind {
                let mut tree = self.js_context.element_tree.borrow_mut();
                if let Some(host) = tree.get_fragment_mut(*fid) {
                    host.kind = Some(kind);
                }
            }
        }
    }

    fn flush_app_events(
        &self,
        boa_context: &mut boa_engine::Context,
        signals: &crate::core::subsystem::FlushSignals<'_>,
    ) -> bool {
        let (platform_events, app_events) = {
            let mut ctx = self.app_context.borrow_mut();
            (
                ctx.platform_event_queue.drain(),
                ctx.app_event_queue.drain(),
            )
        };
        if platform_events.is_empty() && app_events.is_empty() {
            return false;
        }

        let need_paint = self.js_context.need_paint.clone();
        let mut subsystems = self.subsystems.borrow_mut();
        for event in &platform_events {
            self.app_context.borrow_mut().dispatch_platform_event(
                boa_context,
                event,
                &need_paint,
                &mut subsystems,
                signals,
            );
        }

        for event in &app_events {
            self.app_context.borrow_mut().dispatch_app_event(
                boa_context,
                event,
                &need_paint,
                &mut subsystems,
                signals,
            );
        }

        true
    }

    /// Drain the render-command batch produced by the last `flush()`, if any.
    /// `AppBackend::worker_loop` calls this after each `pump()` to ship the
    /// batch to main via `HostMsg::RenderCommands`. Returns `None` if no
    /// paint happened this flush (or already drained).
    pub fn take_pending_render_batch(&self) -> Option<Vec<RenderCommand>> {
        self.pending_render_batch.borrow_mut().take()
    }

    /// Drain the pending-mutation queue and invoke each mutation via the
    /// reactive store. The per-store `{get, set}` JsObject is built inside
    /// [`crate::core::edgy::reactive::SharedReactive::invoke_mutation_by_id`]
    /// only for `Js`-variant closures, so here we pass only the user args.
    /// Invocations run against the **mounted** store (the tree's store), so
    /// declaration atoms touched by the mutation's closure materialize there;
    /// engine-owned atoms route to their owner either way.
    fn flush_pending_mutations(&self, boa_context: &mut boa_engine::Context) -> bool {
        let invs = self.js_context.mutation_queue.borrow_mut().drain();
        if invs.is_empty() {
            return false;
        }
        let mounted = self.js_context.element_tree.store();
        for inv in invs {
            let args = inv.args.to_js_args(boa_context);
            // A failed invocation (e.g. a watch loop rejected a write, or user
            // code threw) must not stall the flush — log and keep draining.
            if let Err(e) = mounted.invoke_mutation(inv.mutation, &args, boa_context) {
                tracing::error!("mutation invocation failed: {e}");
            }
        }
        true
    }
}
