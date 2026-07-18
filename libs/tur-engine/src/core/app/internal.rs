use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use boa_engine::context::time::Clock;
use boa_engine::object::JsObject;
use boa_engine::{js_string, JsValue};

use crate::core::app::TurAppContext;
use crate::core::async_::AsyncExecutor;
use crate::core::reactive::Source;
use crate::core::bridge::TurJobExecutor;
use crate::core::bridge::TurJsContext;
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
    /// `ClipboardWriteHandler` to perform async platform work without
    /// blocking the sync flush loop.
    pub(crate) async_executor: Rc<AsyncExecutor>,
    /// Engine-owned `viewportSize$` reactive source handle. `None` only until
    /// `TurEngineBuilder::build` creates it (right after `new`). Synced each
    /// frame in `flush` from `app_context.size`.
    pub(crate) viewport_size: Option<Source<JsValue>>,
    /// Last `(width, height)` pushed into `viewport_size` — guards against
    /// spurious stale marking (`set_source` compares `JsValue` by object
    /// identity, so a fresh `{w,h}` object would otherwise dirty every frame).
    pub(crate) last_viewport: Cell<(f64, f64)>,
    /// Plugin-registered flush subsystems. Each is `flush`-ed once per
    /// `flush()` call (= once per frame), in registration order, before
    /// `flush_reactive`. The same `Rc<RefCell<…>>` is shared with
    /// [`PluginContext`](crate::core::plugin::PluginContext) so plugins can
    /// push into the vec during `register`.
    pub(crate) subsystems: Rc<RefCell<Vec<Box<dyn Subsystem>>>>,
}

impl TurAppInternal {
    pub fn new(
        renderer: Box<dyn Renderer>,
        font_loader: Box<dyn FontLoader>,
        executor: Rc<TurJobExecutor>,
        clock: std::rc::Rc<dyn Clock>,
    ) -> Self {
        use crate::core::elements::NodeTree;
        use crate::core::mutation::PendingMutationInvocationQueue;
        use crate::core::focus::FocusManager;
        use crate::core::reactive::Store;
        use crate::core::resource::ResourceMap;

        let mutation_queue = Rc::new(RefCell::new(PendingMutationInvocationQueue::new()));
        let focus_manager = Rc::new(RefCell::new(FocusManager::new()));
        let dirty = Rc::new(Cell::new(false));
        let need_paint = Rc::new(Cell::new(false));
        let resource_map = Rc::new(RefCell::new(ResourceMap::default()));

        let async_executor = Rc::new(AsyncExecutor::new(clock.clone()));

        let store = Store::new(dirty.clone());
        let element_tree = NodeTree::new(store.clone());

        let js_context = TurJsContext::new(
            element_tree.clone(),
            mutation_queue.clone(),
            focus_manager.clone(),
            dirty,
            need_paint,
            resource_map.clone(),
            store,
            async_executor.clone(),
        );

        // Share the capability registry between the JS context (bridge fns)
        // and the app context (handlers via HandlerContext). Both hold the
        // same `Rc<RefCell<HashMap>>` via the `Capabilities` view clone.
        let capabilities = js_context.capability();

        let app_context = TurAppContext::new(
            element_tree,
            mutation_queue,
            focus_manager,
            resource_map,
            renderer,
            font_loader,
            async_executor.clone(),
            capabilities,
            clock,
        );

        Self {
            js_context,
            app_context: Rc::new(RefCell::new(app_context)),
            executor,
            async_executor,
            viewport_size: None,
            last_viewport: Cell::new((-1.0, -1.0)),
            subsystems: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn flush(
        &self,
        boa_context: &mut boa_engine::Context,
    ) -> Result<FrameOutcome, TurError> {
        let mut needs_render = false;
        let mut subsystems_ticked = false;
        // Aggregates across all iterations of the fixed-point loop. Once any
        // subsystem requested a frame, the post-loop schedule decision must
        // honour it; once any subsystem dirtied state, the post-loop
        // `need_paint` decision must honour it.
        let mut subsystem_request_frame = false;

        loop {
            // Drive spawned Rust futures one poll step. Completions they
            // produce (settle-JsPromise closures) are drained right after,
            // before boa's microtask drain — so PromiseJobs enqueued by
            // `resolvers.resolve.call(...)` run in the same iteration.
            let async_progress = self.async_executor.tick();
            self.async_executor.drain_completions(boa_context);

            let handled_events = self.flush_app_events();

            // Keep the engine-owned `viewportSize$` atom in sync with the
            // current canvas size (updated by `ResizeHandler` via `cx.size`).
            // Runs before `flush_reactive` so subscribers re-layout in-frame.
            self.sync_viewport_size(boa_context);

            // Subsystem tick — runs once per `flush()` call (= once per
            // frame), in registration order. Each subsystem owns its own
            // clock + state; we feed it the boa context and aggregate the
            // outcomes. Dirtied outcomes force another iteration (the loop
            // continuation check below); request_frame outcomes carry
            // forward into the post-loop schedule decision.
            //
            // Animation (registered via `tur-animation::TurAnimationPlugin`)
            // is the canonical example: it ticks active
            // `AnimationController`s once per frame here, enqueuing
            // `onTick`/`onEnd` mutations that fire later in
            // `flush_pending_mutations`.
            let subsystem_dirtied = if !subsystems_ticked {
                subsystems_ticked = true;
                let mut cx = crate::core::subsystem::SubsystemFlushContext {
                    boa: boa_context,
                };
                let mut dirtied = false;
                for sub in self.subsystems.borrow_mut().iter_mut() {
                    let outcome = sub.flush(&mut cx);
                    if outcome.dirtied {
                        dirtied = true;
                    }
                    if outcome.request_frame {
                        subsystem_request_frame = true;
                    }
                }
                dirtied
            } else {
                false
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
        let schedule = if subsystem_request_frame
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

    /// Build a `{width, height}` JS object (CSS pixels) — the value shape of
    /// the `viewportSize$` atom. Consumed by `build` (initial value) and
    /// `sync_viewport_size` (per-resize update).
    pub(crate) fn viewport_js(
        boa: &mut boa_engine::Context,
        width: f64,
        height: f64,
    ) -> JsValue {
        let obj = JsObject::with_object_proto(boa.intrinsics());
        let _ = obj.create_data_property(js_string!("width"), JsValue::from(width), boa);
        let _ = obj.create_data_property(js_string!("height"), JsValue::from(height), boa);
        obj.into()
    }

    /// Push the current canvas size into the `viewportSize$` atom if it has
    /// changed since the last sync. The `last_viewport` guard is essential:
    /// `set_source` compares `JsValue` by object identity, so rebuilding the
    /// `{width, height}` object every frame would mark the atom stale and
    /// trigger a spurious re-layout on every frame.
    fn sync_viewport_size(&self, boa: &mut boa_engine::Context) {
        let Some(src) = self.viewport_size else {
            return;
        };
        let (width, height) = self.app_context.borrow().size;
        if (width, height) == self.last_viewport.get() {
            return;
        }
        self.last_viewport.set((width, height));
        let value = Self::viewport_js(boa, width, height);
        self.js_context.store.bridge().set_source(src, value);
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

    fn flush_app_events(&self) -> bool {
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

        for event in &platform_events {
            self.app_context
                .borrow_mut()
                .dispatch_platform_handlers(event, &self.js_context.need_paint);
        }

        for event in &app_events {
            self.app_context
                .borrow_mut()
                .dispatch_app_handlers(event, &self.js_context.need_paint);
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
