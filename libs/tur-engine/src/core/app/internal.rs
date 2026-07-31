use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use boa_engine::context::time::Clock;

use crate::core::app::TurAppContext;
use crate::core::async_::{AsyncExecutor, TurJobExecutor};
use crate::core::js_runtime::TurJsContext;
use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::subsystem::Subsystem;

use crate::core::fonts::FontLoader;
use crate::core::render::Renderer;
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
    /// Wake after the given delay (e.g. until the next caret-blink toggle),
    /// then render. Used when no animation is active but a timed paint is.
    After(Duration),
}

/// Outcome of a single [`TurAppInternal::flush`] / `run_frame` call.
#[derive(Debug, Clone, Copy)]
pub struct FrameOutcome {
    /// Whether a new frame was actually rendered this call.
    pub rendered: bool,
    /// How the caller should schedule the next frame.
    pub schedule: NextFrame,
}

pub struct TurAppInternal {
    pub(crate) js_context: TurJsContext,
    pub(crate) app_context: Rc<RefCell<TurAppContext>>,
    pub(crate) executor: Rc<TurJobExecutor>,
    /// Engine-owned async executor. Drives spawned Rust futures via
    /// [`AsyncExecutor::tick`] inside `flush`, with real wakers (backed by
    /// `async_task`). Used by host bridge fns (clipboard, http) and by
    /// `ClipboardWriteSubsystem` to perform async platform work without
    /// blocking the sync flush loop.
    pub(crate) async_executor: Rc<AsyncExecutor>,
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
}

impl TurAppInternal {
    pub fn new(
        renderer: Box<dyn Renderer>,
        font_loader: Box<dyn FontLoader>,
        executor: Rc<TurJobExecutor>,
        clock: std::rc::Rc<dyn Clock>,
    ) -> Self {
        use crate::core::elements::NodeTree;
        use crate::core::edgy::mutation::PendingMutationInvocationQueue;
        use crate::core::focus::FocusManager;
        use crate::core::image_resource::ImageResourceMap;
        use crate::core::edgy::reactive::Store;

        let mutation_queue = Rc::new(RefCell::new(PendingMutationInvocationQueue::new()));
        let focus_manager = Rc::new(RefCell::new(FocusManager::new()));
        let dirty = Rc::new(Cell::new(false));
        let need_paint = Rc::new(Cell::new(false));
        let image_resource_map = Rc::new(RefCell::new(ImageResourceMap::default()));

        let async_executor = Rc::new(AsyncExecutor::new(clock.clone()));

        let store = Store::new(dirty.clone());
        let element_tree = NodeTree::new(store.clone());

        let js_context = TurJsContext::new(
            element_tree.clone(),
            mutation_queue.clone(),
            focus_manager.clone(),
            dirty,
            need_paint,
            image_resource_map.clone(),
            store.clone(),
            async_executor.clone(),
        );

        // Share the capability registry between the JS context (bridge fns)
        // and the app context (subsystems via SubsystemFlushContext). Both hold the
        // same `Rc<RefCell<HashMap>>` via the `Capabilities` view clone.
        let capabilities = js_context.capability();

        let app_context = TurAppContext::new(
            element_tree,
            mutation_queue,
            focus_manager,
            image_resource_map,
            renderer,
            font_loader,
            async_executor.clone(),
            capabilities,
            clock,
            store,
        );

        Self {
            js_context,
            app_context: Rc::new(RefCell::new(app_context)),
            executor,
            async_executor,
            subsystems: Rc::new(RefCell::new(Vec::new())),
            frame_id: Cell::new(0),
        }
    }

    pub fn flush(
        &self,
        boa_context: &mut boa_engine::Context,
    ) -> Result<FrameOutcome, TurError> {
        let mut needs_render = false;
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
        // `cx.request_next_frame`). `sub_dirty` is taken after each iteration
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
            // Drive spawned Rust futures one poll step. Completions they
            // produce (settle-JsPromise closures) are drained right after,
            // before boa's microtask drain — so PromiseJobs enqueued by
            // `resolvers.resolve.call(...)` run in the same iteration.
            let async_progress = self.async_executor.tick();
            self.async_executor.drain_completions(boa_context);

            let handled_events = self.flush_app_events(boa_context, &signals);

            // Pre-layout subsystem flush — runs every fixed-point iteration,
            // in registration order, BEFORE the layout step. Each subsystem
            // owns its own clock + state; time-driven ones self-gate via
            // `cx.frame_id()` so the clock advances at most once per frame.
            // Subsystems push intent back via `cx.mark_dirty()` /
            // `cx.request_paint()` / `cx.request_next_frame()` instead of
            // returning an outcome.
            //
            // Animation (registered via `tur-animation::TurAnimationPlugin`)
            // is the canonical example: it ticks active
            // `AnimationController`s once per frame here (gated by frame_id),
            // enqueuing `onTick`/`onEnd` mutations that fire later in
            // `flush_pending_mutations`, and calls `request_next_frame()`
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
                    renderer: ctx.renderer.as_mut(),
                    screen: &mut ctx.screen,
                    need_paint: &need_paint,
                    async_executor: &ctx.async_executor,
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
        let (reactive_changed, dirty_element_ids) = self.flush_reactive(boa_context);

        // LazyList remount now happens *inside* `perform_layout` (it uses
        // the real viewport from constraints), so there is no separate
        // pre-layout remount pass here.
        let dirty =
            self.js_context.dirty.take() || self.js_context.need_paint.take() || reactive_changed || subsystem_dirtied;
        if dirty {
            needs_render = true;
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
                renderer: ctx.renderer.as_mut(),
                screen: &mut ctx.screen,
                need_paint: &need_paint,
                async_executor: &ctx.async_executor,
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
        // PromiseJobs fire `.then` callbacks which may call bridge fns that
        // `spawn_detached` more Rust futures — those land in
        // `async_executor.ready` and are caught by the `async_progress`
        // termination check on the next iteration, keeping the fixed-point
        // loop alive.
            let jobs_run = self.executor.drain(boa_context).unwrap_or(0);
            let new_dirty = self.js_context.dirty.get() || self.js_context.need_paint.get();
            // Quiescence: no events, no mutations, no dirty state, no async
            // task was polled, no microtasks ran. We deliberately do NOT
            // check `has_pending()` here — a task waiting on a `sleep` timer
            // is not immediately-available work. The `schedule` decision
            // below uses `has_pending` + `next_timer_delay` to decide when
            // to wake the engine next.
            if !handled_events
                && !handled_mutations
                && !new_dirty
                && !async_progress
                && jobs_run == 0
            {
                break;
            }
        }

        if needs_render {
            self.app_context.borrow_mut().render();
            if let Err(e) = self.app_context.borrow_mut().renderer.present() {
                tracing::error!("present failed: {e}");
                return Err(TurError::Render(e.to_string()));
            }
        }

        // Decide how the caller should schedule the next frame.
        //
        // - `Vsync` (continuous): a subsystem requested a frame (e.g. an
        //   animation is running), or a Rust async task is live without a
        //   timer deadline (e.g. clipboard/http futures awaiting external
        //   wake-up). Animations need smooth 60fps; subsystems like audio
        //   need polling; timer-less async tasks need polling each frame.
        // - `After(d)`: nothing continuous is pending, but an async `sleep`
        //   deadline is outstanding (driving a `launch` coroutine or a plain
        //   `sleep().then(...)`). Wake at the deadline rather than polling.
        // - `Idle`: nothing time-driven is pending — the loop can stop
        //   until the next platform input arrives.
        let async_pending = self.async_executor.has_pending();
        let async_timer_delay = self.async_executor.next_timer_delay();
        let schedule = if sub_request_frame.get()
            || (async_pending && async_timer_delay.is_none())
        {
            NextFrame::Vsync
        } else if let Some(delay) = async_timer_delay {
            NextFrame::After(delay)
        } else {
            NextFrame::Idle
        };

        Ok(FrameOutcome {
            rendered: needs_render,
            schedule,
        })
    }

    /// Drain the reactive store and mark affected tree nodes dirty via the
    /// subscriber graph. Returns `(reactive_changed, dirty_element_ids)`:
    /// the element ids whose subscribed atoms changed this flush — used by
    /// the flush loop to fire `on_updated` lifecycle hooks after layout.
    fn flush_reactive(
        &self,
        boa_context: &mut boa_engine::Context,
    ) -> (bool, Vec<ElementNodeId>) {
        let store = self.js_context.store.clone();
        let flush_engine = store.flush_engine();
        if !flush_engine.has_pending() {
            return (false, Vec::new());
        }
        let dirties = flush_engine.flush();
        if dirties.is_empty() {
            return (false, Vec::new());
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

    /// Drain the pending-mutation queue and invoke each mutation via the
    /// reactive store, prepending the `{get, set}` context object. No element
    /// tree access is needed: every entry is a self-contained `(Mutation, args)`.
    fn flush_pending_mutations(&self, boa_context: &mut boa_engine::Context) -> bool {
        let invs = self.js_context.mutation_queue.borrow_mut().drain();
        if invs.is_empty() {
            return false;
        }
        let store = self.js_context.store.clone();
        let ctx_obj = store
            .ctx_object(boa_context)
            .ok()
            .map(boa_engine::JsValue::from);
        for inv in invs {
            let mut args: Vec<boa_engine::JsValue> = Vec::new();
            if let Some(o) = &ctx_obj {
                args.push(o.clone());
            }
            args.extend(inv.args.to_js_args(boa_context));
            let _ = store.invoke_mutation(inv.mutation, &args, boa_context);
        }
        true
    }
}
