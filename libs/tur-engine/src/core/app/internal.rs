use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use boa_engine::context::time::FixedClock;
use boa_engine::object::JsObject;
use boa_engine::{js_string, JsValue};

use crate::core::app::TurAppContext;
use crate::core::async_::{AsyncExecutor, AsyncRuntime};
use crate::core::reactive::Source;
use crate::core::bridge::TurJobExecutor;
use crate::core::bridge::TurJsContext;
use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::event::AppEvent;
use crate::core::focus::{self, helper};
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
    /// then render. Used when no animation is active but a timed redraw is.
    After(Duration),
}

/// Outcome of a single [`TurAppInternal::flush`] / `spawn_loop_once` call.
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
    pub(crate) needs_draw: Rc<Cell<bool>>,
    pub(crate) executor: Rc<TurJobExecutor>,
    /// Engine-owned async executor. Drives spawned Rust futures via
    /// [`AsyncExecutor::tick`] inside `flush`, with real wakers (backed by
    /// `async_task`). Used by host bridge fns (clipboard, http) and by
    /// `ClipboardWriteHandler` to perform async platform work without
    /// blocking the sync flush loop.
    pub(crate) async_executor: Rc<AsyncExecutor>,
    /// Portable async-runtime hooks (wall-clock `now()`, future
    /// `spawn_blocking`/timer). Injected by the embedder via
    /// `TurEngineBuilder::async_runtime`. Held here for future use
    /// (timer scheduling, wall-clock timestamps); the current
    /// clipboard/http bridge fns don't need it.
    #[allow(dead_code)]
    pub(crate) async_runtime: Rc<dyn AsyncRuntime>,
    /// Engine-owned `viewportSize$` reactive source handle. `None` only until
    /// `TurEngineBuilder::build` creates it (right after `new`). Synced each
    /// frame in `flush` from `app_context.size`.
    pub(crate) viewport_size: Option<Source<JsValue>>,
    /// Last `(width, height)` pushed into `viewport_size` — guards against
    /// spurious stale marking (`set_source` compares `JsValue` by object
    /// identity, so a fresh `{w,h}` object would otherwise dirty every frame).
    pub(crate) last_viewport: Cell<(f64, f64)>,
    /// Last caret-blink half-cycle rendered. The caret visibility is a pure
    /// modulo of the deterministic clock (`now_ms / 530`); we only need to
    /// redraw when that half-cycle flips, so we compare against this rather
    /// than redrawing every frame while an editable holds focus.
    pub(crate) last_blink_half: Cell<Option<u64>>,
}

impl TurAppInternal {
    pub fn new(
        renderer: Box<dyn Renderer>,
        font_loader: Box<dyn FontLoader>,
        executor: Rc<TurJobExecutor>,
        clock: std::rc::Rc<FixedClock>,
        async_runtime: Rc<dyn AsyncRuntime>,
    ) -> Self {
        use crate::core::elements::NodeTree;
        use crate::core::edgy_event::PendingMutationInvocationQueue;
        use crate::core::focus::FocusManager;
        use crate::core::reactive::Store;
        use crate::core::resource::ResourceMap;

        let mutation_queue = Rc::new(RefCell::new(PendingMutationInvocationQueue::new()));
        let focus_manager = Rc::new(RefCell::new(FocusManager::new()));
        let dirty = Rc::new(Cell::new(false));
        let resource_map = Rc::new(RefCell::new(ResourceMap::default()));

        let store = Store::new(dirty.clone());
        let element_tree = NodeTree::new(store.clone());

        let js_context = TurJsContext::new(
            element_tree.clone(),
            mutation_queue.clone(),
            focus_manager.clone(),
            dirty.clone(),
            resource_map.clone(),
            store,
        );

        let app_context = TurAppContext::new(
            element_tree,
            mutation_queue,
            focus_manager,
            resource_map,
            renderer,
            font_loader,
            clock,
        );

        let needs_draw = Rc::new(Cell::new(false));

        let async_executor = Rc::new(AsyncExecutor::new());

        // Expose the engine's async executor as a capability so capability-
        // using bridge fns (tur-net's `request`, tur-clipboard's read/write)
        // can spawn futures without capturing state in `unsafe` closures.
        // Plugins that inject their own capabilities (Http, Clipboard) sit
        // on top of this; the executor is engine-owned and always present.
        js_context
            .insert_capability::<Rc<AsyncExecutor>>(async_executor.clone());

        Self {
            js_context,
            app_context: Rc::new(RefCell::new(app_context)),
            needs_draw,
            executor,
            async_executor,
            async_runtime,
            viewport_size: None,
            last_viewport: Cell::new((-1.0, -1.0)),
            last_blink_half: Cell::new(None),
        }
    }

    pub fn flush(
        &self,
        boa_context: &mut boa_engine::Context,
    ) -> Result<FrameOutcome, TurError> {
        let mut needs_render = false;
        let mut animation_ticked = false;

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

            let animation_did_update = if !animation_ticked {
                animation_ticked = true;
                self.tick_animations(boa_context)
            } else {
                self.js_context.animation_manager.borrow().has_active()
            };

        // Reactive flush: drain the store, expand dirty atoms, and dispatch
        // `do_update(dirties)` to the mounted edgy root. This may mutate
        // the ElementTree, which sets `dirty`/`needs_draw` for the next
        // layout pass.
        let (reactive_changed, dirty_element_ids) = self.flush_reactive(boa_context);

        // LazyList remount now happens *inside* `perform_layout` (it uses
        // the real viewport from constraints), so there is no separate
        // pre-layout remount pass here.
        let dirty =
            self.js_context.dirty.take() || self.needs_draw.take() || animation_did_update || reactive_changed;
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
        self.flush_focus_notifications();
        let handled_mutations = self.flush_pending_mutations(boa_context);
            // Run boa microtasks (PromiseJobs, GenericJobs, AsyncJobs,
            // ClockJobs). PromiseJobs fire `.then` callbacks which may call
            // bridge fns that `spawn_detached` more Rust futures — those
            // land in `async_executor.ready` and are caught by the
            // `has_pending`/`async_progress` termination check, keeping
            // the fixed-point loop alive.
            let jobs_run = self.executor.drain(boa_context).unwrap_or(0);
            let new_dirty = self.js_context.dirty.get() || self.needs_draw.get();
            let async_pending = self.async_executor.has_pending();
            // Quiescence requires: no events handled, no mutations handled,
            // no reactive dirty, no microtasks just ran (pre-existing latent
            // bug — a microtask that enqueues another without doing set/event
            // would terminate early), and no pending async work.
            if !handled_events
                && !handled_mutations
                && !new_dirty
                && !async_progress
                && jobs_run == 0
                && !async_pending
            {
                break;
            }
        }

        let animation_active = self
            .js_context
            .animation_manager
            .borrow()
            .has_active();
        if animation_active {
            self.needs_draw.set(true);
        }

        // Decide how the caller should schedule the next frame.
        //
        // - `Vsync` (continuous): an animation is running or a Rust async
        //   task is live. Animations need smooth 60fps; async tasks need
        //   polling each frame.
        // - `After(d)`: nothing continuous is pending, but either a JS timer
        //   (setTimeout/setInterval) is outstanding or an editable holds
        //   focus. We wake precisely at the sooner of the timer deadline and
        //   the next caret-blink toggle — never polling at vsync while a
        //   multi-second interval sits outstanding.
        // - `Idle`: nothing time-driven is pending — the loop can stop until
        //   the next platform input arrives (the embedder re-arms it via the
        //   wake hook installed on `TurApp`).
        let async_pending = self.async_executor.has_pending();
        let timers_pending = self.executor.has_pending_clock_jobs();
        let mut schedule = if animation_active || async_pending {
            NextFrame::Vsync
        } else if timers_pending {
            // Wake at the soonest pending timer deadline (one frame) rather
            // than polling at vsync while e.g. a 5s interval is outstanding.
            let now = boa_context.clock().now();
            match self.executor.next_clock_job_delay(now) {
                Some(delay) => NextFrame::After(delay),
                None => NextFrame::Vsync,
            }
        } else {
            NextFrame::Idle
        };

        if self.focused_is_editable() {
            let now_ms = self.app_context.borrow().shell.now().as_millis() as u64;
            let half = now_ms / focus::CARET_BLINK_HALF_PERIOD_MS;
            if Some(half) != self.last_blink_half.get() {
                // The blink phase flipped since our last render — paint so the
                // caret shows/hides. Subsequent toggles are driven by the
                // `After` deadline below.
                needs_render = true;
                self.last_blink_half.set(Some(half));
            }
            let blink_delay = Duration::from_millis(
                focus::CARET_BLINK_HALF_PERIOD_MS - (now_ms % focus::CARET_BLINK_HALF_PERIOD_MS),
            );
            // Wake at the sooner of the existing schedule and the blink toggle.
            schedule = match schedule {
                NextFrame::Idle => NextFrame::After(blink_delay),
                NextFrame::After(d) => NextFrame::After(d.min(blink_delay)),
                NextFrame::Vsync => NextFrame::Vsync,
            };
        } else {
            // Reset so the next focus re-renders immediately (first half is
            // always "visible", so the half comparison forces a draw).
            self.last_blink_half.set(None);
        }

        if needs_render {
            self.app_context.borrow_mut().render();
            if let Err(e) = self.app_context.borrow_mut().renderer.present() {
                tracing::error!("present failed: {e}");
                return Err(TurError::Render(e.to_string()));
            }
        }
        Ok(FrameOutcome {
            rendered: needs_render,
            schedule,
        })
    }

    /// True if the currently-focused element is an `EditableTextElement`.
    /// Used by `flush` to schedule blink-timed redraws (waking at each caret
    /// toggle) instead of redrawing every frame while an editable holds focus.
    fn focused_is_editable(&self) -> bool {
        let tree = self.js_context.element_tree.borrow();
        let focus = self.js_context.focus_manager.borrow();
        helper::focused_is_editable(&tree, &focus)
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

    fn tick_animations(&self, boa_context: &mut boa_engine::Context) -> bool {
        let now_ms = self.app_context.borrow().shell.now().as_millis() as u64;
        let mut mgr = self.js_context.animation_manager.borrow_mut();
        mgr.tick_controllers(now_ms, boa_context);
        let has_active = mgr.has_active();
        drop(mgr);
        has_active
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
                .dispatch_platform_handlers(event, &self.needs_draw);
        }

        for event in &app_events {
            if matches!(event, AppEvent::RequestDraw) {
                self.needs_draw.set(true);
            }
            self.app_context
                .borrow_mut()
                .dispatch_app_handlers(event, &self.needs_draw);
        }

        true
    }

    /// Resolve pending focus/blur notifications recorded by `FocusManager`.
    /// Delegates to the focus domain, which maps each pending id to its
    /// `Focusable` element (if any) and enqueues the `on_focus` / `on_blur`
    /// mutation. Runs before `flush_pending_mutations` so focus callbacks
    /// fire in the same pass.
    fn flush_focus_notifications(&self) {
        let tree = self.js_context.element_tree.borrow();
        let mut focus = self.js_context.focus_manager.borrow_mut();
        let mut queue = self.js_context.mutation_queue.borrow_mut();
        focus.flush_pending(&tree, &mut queue);
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
