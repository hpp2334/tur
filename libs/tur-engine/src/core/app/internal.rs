use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::app::TurAppContext;
use crate::core::app::frame_stats::FrameTiming;
use crate::core::clock::Clock;
use crate::core::element::{FragmentNodeId, NodeId};
use crate::core::instance::InstanceContext;
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
    pub(crate) instance: InstanceContext,
    pub(crate) app_context: Rc<RefCell<TurAppContext>>,
    /// Worker-thread scheduler. Bridges grab it via
    /// [`PluginRegisterContext::worker_ctx`] / [`SubsystemFlushContext::worker_ctx`]
    /// and call `spawn_local(fut)` to drive async work (clipboard reads,
    /// http requests, sleep futures). The driver's `sleep` returns a
    /// platform-specific `Sleep(BoxFuture)`.
    #[allow(dead_code)]
    pub(crate) worker_ctx: WorkerContext,
    /// Plugin-registered flush subsystems — populated **once** by
    /// `build_worker_backend` (moved from the register-phase
    /// [`PluginRegisterContext`](crate::core::plugin::PluginRegisterContext)
    /// collector after the last plugin registers) and immutable for the
    /// instance's lifetime; no registration path survives the builder.
    /// Each is `flush`-ed **every fixed-point iteration** of `flush()`
    /// (possibly several times per frame), in registration order, before
    /// `flush_reactive`. Time-driven subsystems self-gate via the
    /// per-`flush()` `frame_id` so the clock advances at most once per
    /// frame. `RefCell` (rather than a plain field) only because
    /// `flush(&self)` needs `&mut` access to tick the subsystems.
    pub(crate) subsystems: RefCell<Vec<Box<dyn Subsystem>>>,
    /// Per-`flush()` epoch exposed to subsystems via
    /// [`crate::core::subsystem::SubsystemFlushContext::frame_id`].
    /// Incremented once at the top of each `flush()` call; stable across the
    /// fixed-point iterations within that call.
    pub(crate) frame_id: Cell<u64>,
    /// Worker → main render-command batch produced by the last `flush()`
    /// that painted. Drained by `HostBackend`'s `worker_loop` and shipped
    /// to main via `HostMsg::RenderCommands`. `None` if no paint happened
    /// this flush (or already drained).
    /// The batch recorded by the last painted flush, plus the flush epoch
    /// it was recorded under (echoed to the host in
    /// `HostMsg::RenderCommands.frame_id` and back in
    /// `WorkerMsg::FrameTiming`, so host-side render-commit timings are
    /// attributable). Drained by `HostBackend`'s `worker_loop`.
    pub(crate) pending_render_batch: RefCell<Option<(Vec<RenderCommand>, u64)>>,
}

/// RAII guard set up at `flush()` entry; clears `in_flush` on drop so the
/// worker is "idle" again for out-of-flush self-wakes. Drop runs on every
/// exit path (normal return or future `?`), guaranteeing `end_flush` pairs
/// with `begin_flush`.
struct FlushGuard<'a>(&'a TurAppInternal);

impl Drop for FlushGuard<'_> {
    fn drop(&mut self) {
        self.0.instance.end_flush();
    }
}

impl TurAppInternal {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        font_context: FontContext,
        font_loader: std::sync::Arc<dyn FontLoader>,
        clock: std::sync::Arc<dyn Clock>,
        capabilities: crate::core::capability::Capabilities,
        worker_ctx: WorkerContext,
        wake_worker: std::sync::Arc<dyn Fn() + Send + Sync>,
        host_tx: crate::core::app::HostTx,
        worker_pools: std::sync::Arc<[crate::core::scheduler::WorkerPoolHandle]>,
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

        // Worker-side image state: metadata (sizes) + next-id counter,
        // bundled in one `ImageManager`. The pixel `Blob` ships to main
        // directly from the `createImageResource` bridge via the shared
        // `host_tx` channel (one `HostMsg::UploadImage` per decode). The
        // worker never retains pixels across a frame boundary.
        let image_manager = Rc::new(RefCell::new(ImageManager::new()));

        // Adapt the shared `Arc<dyn Clock>` to the `Rc<dyn Clock>` that
        // `FrameEnv` expects (per-instance + worker-side only, never shared
        // across threads). ClockProxy is a Sized adapter that delegates to
        // the Arc.
        let clock_rc: Rc<dyn Clock> = Rc::new(crate::core::runtime::ClockProxy(clock));

        let store = Store::new(dirty.clone());
        // The instance-owned tree: created at build, born-bound to the
        // INSTANCE store (the engine-created store handed to the module's
        // `start({ store })`). It exists for the instance's whole life —
        // module teardown clears its ROOT (draining the lifecycle), the next
        // module mounts into the same tree. A `mount(store, view)` with an
        // explicit store swaps the binding in place.
        let element_tree = NodeTree::new(store.clone());

        // Frame statistics — one probe per instance, shared by the flush
        // loop (writer) and the `turDevTool` bridge (reader).
        let frame_stats = Rc::new(crate::core::app::FrameStats::default());

        let instance = InstanceContext::new(
            element_tree.clone(),
            mutation_queue.clone(),
            focus_manager.clone(),
            dirty,
            need_paint,
            image_manager.clone(),
            host_tx,
            store.clone(),
            worker_ctx.clone(),
            wake_worker.clone(),
            capabilities,
            worker_pools,
            frame_stats,
        );

        // Share the capability registry between the JS context (bridge fns)
        // and the app context (subsystems via SubsystemFlushContext). Both hold the
        // same `Rc<RefCell<HashMap>>` via the `Capabilities` view clone.
        let capabilities = instance.capability();

        let app_context = TurAppContext::new(
            element_tree,
            mutation_queue,
            focus_manager,
            image_manager,
            font_context,
            font_loader,
            worker_ctx.clone(),
            capabilities,
            clock_rc,
        );

        Self {
            instance,
            app_context: Rc::new(RefCell::new(app_context)),
            worker_ctx,
            subsystems: RefCell::new(Vec::new()),
            frame_id: Cell::new(0),
            pending_render_batch: RefCell::new(None),
        }
    }

    /// Run one flush — the engine's fixed-point convergence loop: events →
    /// pre-layout subsystems → reactive flush → layout → post-layout
    /// subsystems → lifecycle → focus → mutations, until quiescence.
    pub fn flush(&self) -> Result<FrameOutcome, TurError> {
        // Enter the flush window: mark in-flush (so out-of-flush self-wakes
        // raised by `request_paint` / `set_dirty` during this flush don't
        // emit redundant `Wake`s) and re-arm the wake coalescing gate for
        // any paint request raised mid-flush (it must emit a fresh wake for
        // the *next* pump). See `TurInstanceContext::begin_flush` / `end_flush`.
        self.instance.begin_flush();
        let _flush_guard = FlushGuard(self);
        let mut needs_paint = false;
        // Frame-stats probe: per-flush timing + counter accumulation (µs,
        // rounded). Times via the engine clock — `std::time::Instant` is
        // unavailable on wasm (see `clock_now_us`). Never drives behavior.
        let clock = self.app_context.borrow().frame_env.clock();
        let flush_start = crate::core::app::frame_stats::clock_now_us(&*clock);
        let mut layout_us: u64 = 0;
        let mut dirty_layout_nodes: u64 = 0;
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
            let handled_events = self.flush_app_events(&signals);

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
                let need_paint = self.instance.need_paint.clone();
                let mut ctx_guard = self.app_context.borrow_mut();
                let ctx: &mut crate::core::app::TurAppContext = &mut ctx_guard;
                let mut cx = crate::core::subsystem::SubsystemFlushContext {
                    element_tree: ctx.element_tree.clone(),
                    focus_manager: ctx.focus_manager.clone(),
                    mutation_queue: ctx.mutation_queue.clone(),
                    platform_event_queue: &mut ctx.platform_event_queue,
                    app_event_queue: &mut ctx.app_event_queue,
                    screen: &mut ctx.screen,
                    need_paint: &need_paint,
                    worker_ctx: &ctx.worker_ctx,
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
            let reactive_changed = self.flush_reactive(frame_id);

            // LazyList remount now happens *inside* `perform_layout` (it uses
            // the real viewport from constraints), so there is no separate
            // pre-layout remount pass here.
            let dirty = self.instance.dirty.take()
                || self.instance.need_paint.take()
                || reactive_changed
                || subsystem_dirtied;
            if dirty {
                needs_paint = true;
                let layout_start = crate::core::app::frame_stats::clock_now_us(&*clock);
                self.app_context
                    .borrow_mut()
                    .layout(self.instance.dirty.clone());
                layout_us += crate::core::app::frame_stats::clock_now_us(&*clock) - layout_start;
                dirty_layout_nodes += self.instance.element_tree.take_layout_count();
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
                let need_paint = self.instance.need_paint.clone();
                let mut ctx_guard = self.app_context.borrow_mut();
                let ctx: &mut crate::core::app::TurAppContext = &mut ctx_guard;
                let mut cx = crate::core::subsystem::SubsystemFlushContext {
                    element_tree: ctx.element_tree.clone(),
                    focus_manager: ctx.focus_manager.clone(),
                    mutation_queue: ctx.mutation_queue.clone(),
                    platform_event_queue: &mut ctx.platform_event_queue,
                    app_event_queue: &mut ctx.app_event_queue,
                    screen: &mut ctx.screen,
                    need_paint: &need_paint,
                    worker_ctx: &ctx.worker_ctx,
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
            // elements, before_destroy for removed elements. Pushed mutations
            // are drained right after.
            self.run_lifecycle_hooks();
            {
                let mut cx = crate::core::view::SharedViewCx::new(self.instance.clone());
                cx.flush_focus_notifications();
            }
            let handled_mutations = self.flush_pending_mutations();
            let new_dirty = self.instance.dirty.get() || self.instance.need_paint.get();
            // Quiescence: no events and no mutations drained this iteration,
            // no dirty state.
            if !handled_events && !handled_mutations && !new_dirty {
                break;
            }
        }

        if needs_paint {
            // Record the paint pass into a `Vec<RenderCommand>`; main
            // applies it to its renderer (`HostBackend::render_batch`).
            let (batch, parts) = self.app_context.borrow_mut().build_render_batch();
            *self.pending_render_batch.borrow_mut() = Some((batch, frame_id));
            self.instance.frame_stats.record_painted(FrameTiming {
                frame_id,
                nodes_walked: parts.nodes_walked,
                ops_recorded: parts.ops_recorded,
                commands_emitted: parts.commands_emitted,
                batch_bytes: parts.batch_bytes,
                dirty_layout_nodes,
                flush_us: crate::core::app::frame_stats::clock_now_us(&*clock)
                    .saturating_sub(flush_start),
                layout_us,
                record_walk_us: parts.walk_us,
                batch_post_us: parts.post_us,
            });
        } else {
            self.instance.frame_stats.record_idle_flush();
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
    /// subscriber graph. Returns whether any subscriber was dirtied.
    ///
    /// Also delivers `watch()` callbacks: due watchers (their watched atom is
    /// dirtied, at most once per `frame_id`) are pushed onto the mutation
    /// queue, so `flush_pending_mutations` invokes them later this iteration
    /// — same rail, same frame, against the mounted store.
    fn flush_reactive(&self, frame_id: u64) -> bool {
        let store = self.instance.store.clone();
        let flush_engine = store.flush_engine();
        if !flush_engine.has_pending() {
            return false;
        }
        let dirties = flush_engine.flush_atoms();
        if dirties.is_empty() {
            return false;
        }

        // Watchers (non-element subscribers) — queue due callbacks before the
        // element work below; the mutation drain later this iteration invokes
        // them with the mounted store's ctx.
        let due_callbacks = store.watch_dispatch().due_callbacks(&dirties, frame_id);
        if !due_callbacks.is_empty() {
            let mut queue = self.instance.mutation_queue.borrow_mut();
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
            let mut tree = self.instance.element_tree.borrow_mut();
            for sub_id in &dirty_subs {
                tree.mark_dirty(NodeId::new(sub_id.as_u64()));
            }
        }

        // Split dirty subscribers into fragments so fragment rebuilds only
        // process dirty fragments (not a full scan).
        let dirty_frag_ids: Vec<FragmentNodeId> = {
            let tree = self.instance.element_tree.borrow();
            dirty_subs
                .iter()
                .filter(|s| tree.is_fragment(NodeId::new(s.as_u64())))
                .map(|s| FragmentNodeId::new(s.as_u64()))
                .collect()
        };

        // Fragment rebuilds (Condition / Each / Switch branch swaps).
        if !dirty_frag_ids.is_empty() {
            self.rebuild_fragments(&dirty_frag_ids);
        }

        !dirty_subs.is_empty()
    }

    /// Fire element lifecycle hooks: `on_mounted` for newly-inserted elements
    /// and `before_destroy` for elements removed since the last pass.
    /// All hooks run after layout (so the mutation queue is drained by the
    /// subsequent `flush_pending_mutations`).
    fn run_lifecycle_hooks(&self) {
        let mut cx = crate::core::view::SharedViewCx::new(self.instance.clone());

        // on_mounted — freshly-inserted elements.
        let mounted_ids = self
            .instance
            .element_tree
            .borrow_mut()
            .take_pending_mounted();
        for id in mounted_ids {
            let mut element = {
                let mut tree = self.instance.element_tree.borrow_mut();
                tree.get_element_mut(id).and_then(|n| n.element.take())
            };
            if let Some(ref mut elem) = element {
                elem.run_on_mounted(&mut cx);
            }
            if let Some(elem) = element {
                let mut tree = self.instance.element_tree.borrow_mut();
                if let Some(node) = tree.get_element_mut(id) {
                    node.element = Some(elem);
                }
            }
        }

        // before_destroy — elements removed since the last pass. The element
        // is already detached from the tree (taken out during destroy), so we
        // just fire the hook and let it drop.
        let destroyed = self
            .instance
            .element_tree
            .borrow_mut()
            .take_pending_destroy();
        for mut elem in destroyed {
            elem.run_before_destroy(&mut cx);
        }
    }

    /// Rebuild dirty fragments (Condition / Each / Switch). Only fragments
    /// whose subscribed atoms are dirty are processed — identified via the
    /// subscriber graph, not a full scan. Each fragment's `perform_update`
    /// resolves the current value and swaps the branch/items if changed.
    fn rebuild_fragments(&self, dirty_frag_ids: &[FragmentNodeId]) {
        let mut cx = crate::core::view::SharedViewCx::new(self.instance.clone());

        for fid in dirty_frag_ids {
            let mut kind = {
                let mut tree = self.instance.element_tree.borrow_mut();
                tree.get_fragment_mut(*fid).and_then(|h| h.kind.take())
            };
            let Some(ref mut k) = kind else { continue };

            // Save old children + parent BEFORE rebuild (perform_update
            // auto-links new children to frag.children via append_child).
            let (old_children, parent) = {
                let tree = self.instance.element_tree.borrow();
                tree.get_fragment(*fid)
                    .map(|f| (f.children.clone(), f.parent))
                    .unwrap_or((Vec::new(), (*fid).into()))
            };

            let new_children = k.perform_update(&mut cx, *fid);

            if let Some(new) = new_children {
                // frag.children now has old + new; replace with just new.
                {
                    let mut tree = self.instance.element_tree.borrow_mut();
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
                let mut tree = self.instance.element_tree.borrow_mut();
                if let Some(host) = tree.get_fragment_mut(*fid) {
                    host.kind = Some(kind);
                }
            }
        }
    }

    fn flush_app_events(&self, signals: &crate::core::subsystem::FlushSignals<'_>) -> bool {
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

        let need_paint = self.instance.need_paint.clone();
        let mut subsystems = self.subsystems.borrow_mut();
        for event in &platform_events {
            self.app_context.borrow_mut().dispatch_platform_event(
                event,
                &need_paint,
                &mut subsystems,
                signals,
            );
        }

        for event in &app_events {
            self.app_context.borrow_mut().dispatch_app_event(
                event,
                &need_paint,
                &mut subsystems,
                signals,
            );
        }

        true
    }

    /// Drain the render-command batch produced by the last `flush()`, if any.
    /// `HostBackend::worker_loop` calls this after each `pump()` to ship the
    /// batch to main via `HostMsg::RenderCommands`. Returns `None` if no
    /// paint happened this flush (or already drained).
    pub fn take_pending_render_batch(&self) -> Option<(Vec<RenderCommand>, u64)> {
        self.pending_render_batch.borrow_mut().take()
    }

    /// Drain the pending-mutation queue and invoke each mutation via the
    /// reactive store with the payload's native args (see
    /// [`crate::core::edgy::mutation::MutationPayload::to_value_args`]).
    /// Invocations run against the **mounted** store (the tree's store), so
    /// atoms touched by the mutation's closure materialize there.
    fn flush_pending_mutations(&self) -> bool {
        let invs = self.instance.mutation_queue.borrow_mut().drain();
        if invs.is_empty() {
            return false;
        }
        let mounted = self.instance.element_tree.store();
        for inv in invs {
            let args = inv.args.to_value_args();
            // A failed invocation (e.g. a watch loop rejected a write, or
            // user code trapped) must not stall the flush — log + report
            // through the runtime-error rail and keep draining.
            if let Err(e) = mounted.invoke_mutation(inv.mutation, &args) {
                tracing::error!("mutation invocation failed: {e}");
                crate::core::app::runtime_error::send_report(
                    &self.instance.host_tx,
                    format!("mutation invocation failed: {e}"),
                    None,
                );
            }
        }
        true
    }

    /// Teardown support: fire the pending lifecycle hooks (notably
    /// `before_destroy` for elements removed by the teardown's
    /// `destroy_subtree`) and drain the mutations they queued — invoked
    /// against the still-bound mounted store. The caller drops the tree
    /// right after: it must not outlive its pending lifecycle work.
    #[allow(dead_code)] // (teardown seam — re-armed with the rut root-lifecycle gate)
    pub(crate) fn drain_teardown_lifecycle(&self) {
        self.run_lifecycle_hooks();
        self.flush_pending_mutations();
    }
}
