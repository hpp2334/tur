//! Backend types: `WorkerBackend` (engine state on the worker thread) and
//! `MainBackend` (the public backend owned by `TurApp`, spawns + dispatches
//! to the worker).
//!
//! ## Architecture
//!
//! - [`WorkerBackend`] is `pub(crate)`: it lives on the worker thread and
//!   owns the boa `Context`, element tree, reactive store, subsystems.
//!   `pump()` runs one flush and produces a `Vec<RenderCommand>` batch
//!   (stored in `TurAppInternal::pending_render_batch`).
//!
//! - [`MainBackend`] is public: `TurApp` owns one. It spawns a worker
//!   thread (via [`crate::core::thread`]) running a `WorkerBackend`,
//!   dispatches input via `futures::channel`, and receives [`MainMsg`]
//!   replies. `MainBackend` owns the main-side [`Renderer`] (passed to
//!   `TurRuntime::app_builder().build(...)`); it applies each
//!   `MainMsg::RenderCommands` batch directly to the renderer — no
//!   `render_sink` callback.
//!
//! ## Async model
//!
//! All channels use `futures::channel` (mpsc + oneshot). The worker thread entry-point wraps
//! an `async fn worker_loop(...)` via `futures::executor::block_on`, so the
//! worker awaits on `worker_rx.recv()` instead of blocking on a Mutex +
//! Condvar. Main-thread `run_loop` and `rpc` are `async fn`; the embedder
//! supplies the runtime (`wasm_bindgen_futures::spawn_local` on wasm,
//! `futures::executor::block_on` on native).

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use boa_engine::{Context, Source};
use futures::StreamExt;

use crate::FocusedState;
use crate::core::app::{FrameOutcome, ModuleError, TurAppInternal, WorkerMsg};
use crate::core::app::{MainMsg, MainRx, MainTx, Reply, WorkerRx, WorkerTx};
use crate::core::async_::TurJobExecutor;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{DevNodeData, NodeTreeSnapshot};
use crate::core::event_bus::EventBus;
use crate::core::image_resource::{ImageResource, ImageResourceId};
use crate::core::platform::CursorBackend;
use crate::core::render::{RenderCommand, RenderCommandBatch, Renderer};
use crate::core::scheduler::WorkerHandle;
use crate::error::TurError;

// ---------------------------------------------------------------------------
// WorkerBackend — engine state on the worker thread (pub(crate))
// ---------------------------------------------------------------------------

/// The engine state container, owned by the worker thread. Holds the boa
/// `Context`, `TurAppInternal`, and the job executor.
///
/// Constructed on the worker thread (via [`build_worker_backend`]) so it
/// can capture `!Send` types like `Rc<dyn Clock>` and `boa::Context` —
/// these never cross threads. Once constructed, [`WorkerBackend::pump`]
/// runs one flush and stores the resulting `Vec<RenderCommand>` batch in
/// `TurAppInternal::pending_render_batch`, where [`MainBackend`]'s
/// `worker_loop` drains it and ships to main.
pub(crate) struct WorkerBackend {
    pub(crate) boa_context: RefCell<Context>,
    pub(crate) internal: TurAppInternal,
    pub(crate) executor: Rc<TurJobExecutor>,
}

impl WorkerBackend {
    pub(crate) fn new(
        boa_context: Context,
        internal: TurAppInternal,
        executor: Rc<TurJobExecutor>,
    ) -> Self {
        Self {
            boa_context: RefCell::new(boa_context),
            internal,
            executor,
        }
    }

    /// Read the latest cursor applied during the last flush (or `None` if
    /// no pointer was over the surface / no cursor change happened).
    pub(crate) fn last_applied_cursor(&self) -> Option<crate::core::platform::Cursor> {
        self.internal
            .app_context
            .borrow()
            .shell
            .last_applied_cursor()
    }

    pub(crate) fn take_pending_render_batch(&self) -> Option<Vec<RenderCommand>> {
        self.internal.take_pending_render_batch()
    }

    fn load_js_inner(&self, source: &str) -> Result<(), ModuleError> {
        let mut boa = self.boa_context.borrow_mut();
        boa.eval(Source::from_bytes(source).with_path(Path::new("bundle.js")))
            .map_err(|e| {
                tracing::error!("JS eval error: {e}");
                ModuleError::Eval(e.to_string())
            })?;
        if let Err(e) = self.executor.drain(&mut boa) {
            tracing::error!("load_js drain error: {e}");
        }
        tracing::info!("load_js: bundle evaluated successfully");
        Ok(())
    }

    fn load_module_inner(&self, source: &str) -> Result<(), ModuleError> {
        let mut boa = self.boa_context.borrow_mut();
        let module = boa_engine::Module::parse(
            Source::from_bytes(source).with_path(Path::new("entry.mjs")),
            None,
            &mut boa,
        )
        .map_err(|e| {
            tracing::error!("module parse error: {e}");
            ModuleError::Parse(e.to_string())
        })?;
        let promise = module.load_link_evaluate(&mut boa);
        if let Err(e) = boa.run_jobs() {
            tracing::error!("module run_jobs error: {e}");
        }
        // Surface module-body rejections as `ModuleError::Eval`. Without
        // this check, errors thrown during evaluation silently vanish
        // (run_jobs clears the rejection) — the engine appears to load
        // successfully but the bundle's render() never runs.
        if let boa_engine::builtins::promise::PromiseState::Rejected(reason) = promise.state() {
            let to_string = reason
                .to_string(&mut boa)
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            tracing::error!("module eval rejected: {to_string}");
            drop(boa);
            return Err(ModuleError::Eval(to_string));
        }
        drop(boa);
        if let Err(e) = self.executor.drain(&mut self.boa_context.borrow_mut()) {
            tracing::error!("load_module drain error: {e}");
        }
        Ok(())
    }

    fn eval_module_inner(&self, source: &str) -> Result<(), ModuleError> {
        let mut boa = self.boa_context.borrow_mut();
        let module = boa_engine::Module::parse(
            Source::from_bytes(source).with_path(Path::new("eval.mjs")),
            None,
            &mut boa,
        )
        .map_err(|e| {
            tracing::error!("eval_module parse error: {e}");
            ModuleError::Parse(e.to_string())
        })?;
        let _promise = module.load_link_evaluate(&mut boa);
        if let Err(e) = boa.run_jobs() {
            tracing::error!("eval_module run_jobs error: {e}");
        }
        drop(boa);
        let _ = self.executor.drain(&mut self.boa_context.borrow_mut());
        Ok(())
    }

    /// Dispatch one [`WorkerMsg`]. RPC variants settle their own `Reply`;
    /// non-RPC variants push state into the worker for the next `pump()`.
    pub(crate) fn handle_worker_msg(&self, msg: WorkerMsg) {
        match msg {
            WorkerMsg::PlatformEvent(event) => {
                self.internal
                    .app_context
                    .borrow_mut()
                    .platform_event_queue
                    .push(event);
            }
            WorkerMsg::Wake => {
                // The worker_loop drives flush via `pump()` (separate method
                // so it can capture the FrameOutcome + ship commands).
            }
            WorkerMsg::LoadModule { source, reply } => {
                let res = self.load_module_inner(&source);
                // The worker owns its paint state: if eval produced paint-
                // worthy state (element creation already self-wakes via
                // `set_dirty`; this also covers a pure reactive `set`),
                // re-arm an idle worker so the bundle renders without an
                // embedder paint request.
                self.wake_if_dirty();
                reply.send(res);
            }
            WorkerMsg::LoadJs { source, reply } => {
                let res = self.load_js_inner(&source);
                self.wake_if_dirty();
                reply.send(res);
            }
            WorkerMsg::EvalModule { source, reply } => {
                let res = self.eval_module_inner(&source);
                self.wake_if_dirty();
                reply.send(res);
            }
            WorkerMsg::EvalJs { source, reply } => {
                // Test-only synchronous JS evaluation. Drains promise jobs
                // + completions so `await`-free side effects settle before
                // the reply fires. If the result is a JS string, returns
                // the string contents (no quotes); otherwise returns the
                // display form (matches the legacy `eval_js` semantics).
                let source_str: &str = &source;
                let mut boa = self.boa_context.borrow_mut();
                let result = boa.eval(boa_engine::Source::from_bytes(source_str));
                let display = match result {
                    Ok(v) => v
                        .as_string()
                        .map(|s| s.to_std_string_escaped())
                        .unwrap_or_else(|| v.display().to_string()),
                    Err(e) => {
                        tracing::error!("EvalJs error: {e}");
                        String::new()
                    }
                };
                let _ = boa.run_jobs();
                drop(boa);
                let _ = self.executor.drain(&mut self.boa_context.borrow_mut());
                reply.send(display);
            }
            WorkerMsg::DevElementTree { reply } => {
                reply.send(self.dev_tool_element_tree());
            }
            WorkerMsg::DevGetElement { id, reply } => {
                reply.send(self.dev_tool_get_element(id));
            }
            WorkerMsg::QueryTreeSnapshot { reply } => {
                reply.send(self.query_tree_snapshot());
            }
            WorkerMsg::WithElement { id, runner } => {
                let tree = self.internal.js_context.element_tree.borrow();
                runner(&tree);
                // `id` is informational only (the closure did its own
                // lookup); reference it so the variant's bind stays useful.
                let _ = id;
            }
            WorkerMsg::QueryFocusedState { reply } => {
                reply.send(self.focused_state());
            }
            WorkerMsg::QueryFocusedElement { reply } => {
                reply.send(self.focused_element());
            }
            WorkerMsg::QueryFocusedCursorRect { reply } => {
                reply.send(self.focused_cursor_rect());
            }
            WorkerMsg::QueryFocusedIsEditable { reply } => {
                reply.send(self.focused_is_editable());
            }
            WorkerMsg::QueryElement { key, reply } => {
                let key_refs: Vec<&str> = key.iter().map(|s| s.as_str()).collect();
                reply.send(self.query_element(&key_refs));
            }
            WorkerMsg::EventBusToJs(bytes) => {
                self.internal.event_bus.emit_to_js(bytes);
                self.internal.js_context.wake_if_idle();
            }
            WorkerMsg::AppEvent(event) => {
                self.push_app_event(event);
            }
            WorkerMsg::Destroy { reply } => {
                reply.send(());
            }
        }
    }

    pub(crate) fn pump(&self) -> Result<FrameOutcome, TurError> {
        // `Wake` is a no-op above; flush is driven here so the outcome can
        // be returned to the worker_loop, which then ships any pending
        // render batch.
        let mut boa = self.boa_context.borrow_mut();
        self.internal.flush(&mut boa)
    }

    fn push_app_event(&self, event: crate::core::app::AppEvent) {
        self.internal
            .app_context
            .borrow_mut()
            .app_event_queue
            .push(event);
    }

    /// After a module/script eval, re-arm an idle worker if the eval left
    /// paint-worthy state (dirty tree / `need_paint`). Coalesced + in-flush
    /// gated by `TurJsContext::wake_if_idle`. Lets the worker self-paint on
    /// load with no embedder paint request.
    fn wake_if_dirty(&self) {
        let js = &self.internal.js_context;
        if js.dirty.get() || js.need_paint.get() {
            js.wake_if_idle();
        }
    }

    #[allow(dead_code)]
    pub(crate) fn event_bus(&self) -> Rc<EventBus> {
        self.internal.event_bus.clone()
    }

    #[allow(dead_code)]
    pub(crate) fn event_bus_handle(&self) -> crate::core::event_bus::EventBusHandle {
        let (h, j) = self.internal.event_bus.queues();
        crate::core::event_bus::EventBusHandle::from_queues(h, j)
    }

    pub(crate) fn focused_state(&self) -> FocusedState {
        FocusedState {
            is_editable: self.focused_is_editable(),
            cursor_rect: self.focused_cursor_rect(),
        }
    }

    pub(crate) fn focused_element(&self) -> Option<ElementNodeId> {
        self.internal.js_context.focus_manager.borrow().focused()
    }

    pub(crate) fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let focused_id = self.focused_element()?;
        let tree = self.internal.js_context.element_tree.borrow();

        let mut abs_x = 0.0f64;
        let mut abs_y = 0.0f64;
        let mut current: Option<NodeId> = Some(focused_id.into());
        while let Some(id) = current {
            let node = tree.get_element(ElementNodeId::new(id.as_u64()))?;
            abs_x += node.computed_layout.offset.x;
            abs_y += node.computed_layout.offset.y;
            current = node.parent;
        }

        let node = tree.get_element(focused_id)?;
        let element = node.element.as_ref()?;
        let (cx, cy, cw, ch) = element.cursor_rect_relative()?;

        Some((abs_x + cx, abs_y + cy, cw, ch))
    }

    pub(crate) fn focused_is_editable(&self) -> bool {
        use crate::core::focus::helper;
        let tree = self.internal.js_context.element_tree.borrow();
        let focus = self.internal.js_context.focus_manager.borrow();
        helper::focused_is_editable(&tree, &focus)
    }

    pub(crate) fn query_element(&self, key: &[&str]) -> Option<NodeId> {
        self.internal
            .js_context
            .element_tree
            .borrow()
            .query_element(key)
    }

    pub(crate) fn dev_tool_element_tree(&self) -> Option<DevNodeData> {
        let tree = self.internal.js_context.element_tree.borrow();
        let root_id = tree.root_element_id()?;
        tree.dev_tool_node(root_id.into())
    }

    pub(crate) fn dev_tool_get_element(&self, id: NodeId) -> Option<DevNodeData> {
        self.internal
            .js_context
            .element_tree
            .borrow()
            .dev_tool_node(id)
    }

    pub(crate) fn query_tree_snapshot(&self) -> NodeTreeSnapshot {
        self.internal
            .js_context
            .element_tree
            .borrow()
            .tree_snapshot()
    }
}

// ---------------------------------------------------------------------------
// MainBackend — TurApp's backend. Owns the worker + RPC plumbing + renderer
// ---------------------------------------------------------------------------

/// Result of dispatching one worker→main [`MainMsg`] through
/// [`MainBackend::apply_msg`] — the single message handler driven by
/// `TurApp::run_loop`.
///
/// `apply_msg` performs every side-effect that is independent of *when* the
/// batch is painted (cursor backend apply, focus-change handler, image
/// upload, event-bus dispatch) and returns this enum so `run_loop` can
/// apply its vsync-aligned render policy:
/// - [`Render`] is buffered for pipelining (latest-wins) and rendered at
///   the next vsync, or flushed at quiescence if no vsync is armed.
/// - [`Frame`] fires `after_frame` and re-arms vsync when scheduled.
/// - [`Closed`] is terminal.
///
/// Because there is exactly one driver, the focus-change handler (and
/// every other side-effect) can never drift between execution paths.
pub(crate) enum MsgOutcome {
    /// Side-effects already applied; the driver should keep draining.
    Continue,
    /// A render-command batch. The driver decides when to paint it.
    Render(RenderCommandBatch),
    /// A completed frame. Terminal for a single-frame advance.
    Frame(FrameOutcome),
    /// The worker's flush errored. Terminal.
    Failed(String),
    /// The worker is gone. Terminal — the driver must stop.
    Closed,
}

/// The public backend owned by `TurApp`. Spawns a worker thread running a
/// [`WorkerBackend`], dispatches input via `futures::channel`, and receives
/// [`MainMsg`] replies.
///
/// ## Async rpc
///
/// All public methods on `MainBackend` are `async fn`. The embedder
/// supplies the runtime — `wasm_bindgen_futures::spawn_local` on wasm
/// (so the JS main thread never blocks), `futures::executor::block_on`
/// on native (so the calling thread parks until the worker replies).
///
/// ## Renderer ownership
///
/// `MainBackend` owns the main-side [`Renderer`] — passed to
/// `TurRuntime::app_builder().renderer(Box<dyn Renderer>, …).build()` and
/// stored here, exactly like `main`'s
/// `app_builder().renderer(Box::new(renderer), …).build()`. Both
/// `MainBackend` and the renderer live on the main thread, so there is no
/// callback indirection: each `MainMsg::RenderCommands` batch is applied
/// directly via [`Self::render_batch`] (renderer only). Resize is
/// driven by the embedder at event-receipt time via
/// [`TurApp::resize`](crate::TurApp::resize) (DOM `ResizeObserver` / winit
/// / JNI), which calls [`Self::resize`] directly and forwards
/// `PlatformEvent::Resize` to the worker for layout — no `MainMsg` round-trip.
///
/// ## Focus-change handler
///
/// The worker emits `MainMsg::CursorChanged` and `MainMsg::FocusedStateChanged`
/// (deduped against the previous frame) alongside the FrameOutcome. `apply_msg`
/// applies the cursor directly to the cursor backend and forwards focus
/// changes to an embedder-installed handler (see [`Self::set_focus_changed_handler`]).
/// The engine retains no focus cache — embedders read focus either from the
/// handler (push) or via [`Self::focused_state`] (async RPC).
pub struct MainBackend {
    worker_tx: WorkerTx,
    /// Wrapped in `RefCell` because `futures::channel::mpsc::UnboundedReceiver::next`
    /// requires `&mut self`, but `MainBackend` is held inside `Rc<TurApp>`
    /// on wasm + android (single-threaded ownership). The borrow is held
    /// across the `next().await` in `run_loop` — safe because the wasm main
    /// thread is single-threaded and `Rc<TurApp>` itself enforces
    /// single-threaded access.
    pub(crate) main_rx: RefCell<MainRx>,
    /// Holds the worker `JoinHandle` alive for the backend's lifetime so
    /// the worker thread (or Web Worker on wasm) doesn't get reclaimed.
    _worker_handle: WorkerHandle,
    /// Cross-thread wake. Called after every main→worker send. No-op on
    /// native; `worker.postMessage(0)` on wasm.
    worker_notify: Rc<dyn Fn()>,
    /// Main-side cursor backend. Worker emits `MainMsg::CursorChanged` on
    /// cursor state change; `apply_msg` applies it directly here. Set via
    /// `set_cursor_backend` (called by embedder after
    /// `app_builder().build(...)`).
    cursor_backend: RefCell<Option<Arc<std::sync::Mutex<dyn CursorBackend + Send + Sync>>>>,
    /// Embedder-installed handler fired from `apply_msg` whenever the
    /// worker ships a deduped `MainMsg::FocusedStateChanged`. The engine
    /// retains no focus cache — embedders that need the value (wasm's
    /// textarea positioning, Android's soft-keyboard sync) register a
    /// handler here; on-demand reads use [`Self::focused_state`] (RPC).
    focus_changed_handler: RefCell<Option<crate::FocusChangedHook>>,
    /// Cross-thread event bus handle. Routes `emit_to_js` via
    /// `WorkerMsg::EventBusToJs` (channel mode).
    event_bus_handle: crate::core::event_bus::EventBusHandle,
    /// Main-side renderer (owned — no sink callback). Worker ships
    /// `MainMsg::RenderCommands` batches; main applies them here.
    renderer: RefCell<Box<dyn Renderer>>,
    /// Main-side image resources — the full `ImageResource` (pixel `Blob`
    /// retained) per worker-assigned id. Inserted on `MainMsg::UploadImage`
    /// (under the worker-assigned id) alongside the GPU upload; retained for
    /// context-loss re-upload. The worker only ever holds the sizes
    /// (`ImageManager`).
    image_resource_map: RefCell<crate::core::image_resource::ImageResourceMap>,
}

impl MainBackend {
    /// Spawn a worker via [`MainScheduler::spawn_worker`]. The factory runs
    /// on the worker thread and constructs the [`WorkerBackend`] (so it can
    /// build `!Send` types like `Rc<dyn Clock>` and `boa::Context`).
    ///
    /// The driver's `spawn_worker` impl sets up thread-locals on the worker
    /// thread, constructs a `WorkerScheduler` for it, passes that to the
    /// factory, and finally drives the factory's returned future (the
    /// engine's `worker_loop`) the way that platform keeps a worker alive
    /// — native blocks the OS thread on its LocalPool; wasm roots the
    /// future on the JS event loop via `spawn_local`. The factory also
    /// receives a worker→main channel sender clone (`main_tx`) so bridges
    /// can ship messages (e.g. `MainMsg::UploadImage` from
    /// `createImageResource`) directly without a staging vec.
    pub(crate) fn new(
        main_sched: crate::core::scheduler::MainScheduler,
        renderer: Box<dyn Renderer>,
        backend_factory: impl FnOnce(
            crate::core::scheduler::WorkerScheduler,
            std::sync::Arc<dyn Fn() + Send + Sync>,
            crate::core::app::MainTx,
        ) -> WorkerBackend
        + Send
        + 'static,
    ) -> Self {
        let (worker_tx, worker_rx) = futures::channel::mpsc::unbounded::<WorkerMsg>();
        let (main_tx, main_rx) = futures::channel::mpsc::unbounded::<MainMsg>();

        // One-shot init signal: worker fires after `backend_factory()` (which
        // runs `plugin.register` + capability replay) completes. Native main
        // blocks on this so `app_builder().build(...)` returning guarantees the
        // worker's plugin-level side effects are observable. On wasm the main
        // thread cannot block — embedders must await an RPC instead.
        #[cfg(not(target_arch = "wasm32"))]
        let (init_tx, init_rx) = std::sync::mpsc::channel::<()>();

        let worker_tx_for_on_push = worker_tx.clone();
        // Clone of the worker→main sender handed to the backend so bridges
        // can ship messages directly (FIFO order is preserved across the
        // shared channel — the bridge enqueues during flush, worker_loop
        // enqueues after flush).
        let main_tx_for_backend = main_tx.clone();
        #[cfg(not(target_arch = "wasm32"))]
        let init_tx = init_tx;
        let worker_handle = main_sched.spawn_worker(Box::new(move |worker_sched| {
            let worker_tx_for_on_push = worker_tx_for_on_push.clone();
            // `Send + Sync` so the flush-driven task waker (which sleep
            // futures register with the test `VirtualClock`, fired
            // cross-thread) can hold an `Arc` clone.
            let wake_worker: std::sync::Arc<dyn Fn() + Send + Sync> =
                std::sync::Arc::new(move || {
                    let _ = worker_tx_for_on_push.unbounded_send(WorkerMsg::Wake);
                });
            let backend = backend_factory(worker_sched.clone(), wake_worker, main_tx_for_backend);
            #[cfg(not(target_arch = "wasm32"))]
            let _ = init_tx.send(());
            // Return the worker's main future; the driver drives it
            // (native blocks on LocalPool; wasm runs the cooperative
            // mini-executor + cross-thread postMessage wake).
            Box::pin(worker_loop(backend, worker_rx, main_tx))
        }));

        // Cross-thread wake callback. No-op on native (mpsc waker unparks
        // the OS thread); `worker.postMessage(0)` on wasm (the only way to
        // kick an idle Web Worker's JS event loop without a sync
        // `Atomics.wait`). Called after every main→worker send below.
        let worker_notify = worker_handle.notify();

        // Native: synchronously wait for the worker to finish init.
        #[cfg(not(target_arch = "wasm32"))]
        init_rx
            .recv()
            .expect("worker thread died during backend_factory");

        Self {
            worker_tx: worker_tx.clone(),
            main_rx: RefCell::new(main_rx),
            _worker_handle: worker_handle,
            worker_notify,
            cursor_backend: RefCell::new(None),
            focus_changed_handler: RefCell::new(None),
            event_bus_handle: crate::core::event_bus::EventBusHandle::from_channel(worker_tx),
            renderer: RefCell::new(renderer),
            image_resource_map: RefCell::new(
                crate::core::image_resource::ImageResourceMap::default(),
            ),
        }
    }

    /// Install the main-side cursor backend. Worker emits
    /// `MainMsg::CursorChanged`; main applies here during `apply_msg`.
    pub fn set_cursor_backend(
        &self,
        backend: Arc<std::sync::Mutex<dyn CursorBackend + Send + Sync>>,
    ) {
        *self.cursor_backend.borrow_mut() = Some(backend);
    }

    /// Install a handler fired from [`Self::apply_msg`] whenever the worker
    /// ships a deduped `MainMsg::FocusedStateChanged`. The engine keeps no
    /// focus cache — embedders obtain focus state either by registering
    /// here (push, on change) or via [`Self::focused_state`] (async RPC,
    /// on demand). Pass `None` to clear.
    pub fn set_focus_changed_handler(&self, handler: Option<crate::FocusChangedHook>) {
        *self.focus_changed_handler.borrow_mut() = handler;
    }

    /// Cross-thread event bus handle (queues mode is unused; the worker's
    /// `EventBus` isn't reachable from main).
    pub fn event_bus_handle(&self) -> crate::core::event_bus::EventBusHandle {
        self.event_bus_handle.clone()
    }

    /// Send a fire-and-forget [`WorkerMsg`] (no Reply slot). Used by
    /// `push_platform_event`, `request_paint`, `push_app_event`,
    /// `emit_to_js`. Wakes the worker cross-thread after the send (no-op on
    /// native; `postMessage` on wasm).
    pub(crate) fn send_worker_msg(&self, msg: WorkerMsg) {
        let _ = self.worker_tx.unbounded_send(msg);
        self.wake_worker();
    }

    /// Fire the cross-thread wake callback. Cheap; safe to call after every
    /// main→worker send. `(&*rc)()` derefs the `Rc<dyn Fn>` to call it.
    fn wake_worker(&self) {
        use std::ops::Deref;
        self.worker_notify.deref()();
    }

    /// Apply a render-command batch to the owned renderer (encode +
    /// present). Called from `TurApp::run_loop` (both the vsync-aligned
    /// pipelining path and the quiescence flush) — single source of truth
    /// for render application.
    pub(crate) fn render_batch(&self, commands: &[RenderCommand]) {
        let mut r = self.renderer.borrow_mut();
        r.render_commands(commands);
        let _ = r.present();
    }

    /// Upload a newly-registered image resource to the owned renderer.
    pub(crate) fn upload_image_resource(&self, id: ImageResourceId, image: &ImageResource) {
        self.renderer.borrow_mut().upload_image_resource(id, image);
    }

    /// Retain a shipped image resource on main (under the worker-assigned
    /// id) — the main-side `ImageResourceMap` is the pixel `Blob` owner,
    /// kept for context-loss re-upload. The worker never retains the Blob.
    pub(crate) fn insert_image_resource(&self, id: ImageResourceId, image: ImageResource) {
        self.image_resource_map
            .borrow_mut()
            .insert_with_id(id, image);
    }

    /// Resize the owned renderer. Called by `TurApp::resize`, which the
    /// embedder invokes at resize-event-receipt time (DOM `ResizeObserver`
    /// / winit / JNI) — event-driven, not per-frame, so no dedup is needed.
    pub(crate) fn resize(&self, logical_width: u32, logical_height: u32, dpr: f64) {
        self.renderer
            .borrow_mut()
            .resize(logical_width, logical_height, dpr);
    }

    /// Pixel readback from the owned renderer (screenshot tests). Returns
    /// `None` if the renderer doesn't support readback.
    pub(crate) fn render_to_pixels(&self) -> Option<Vec<u8>> {
        self.renderer.borrow_mut().render_to_pixels()
    }

    /// Borrow the worker→main channel sender. Used by call sites that
    /// build a `WorkerMsg` carrying a closure / reply slot directly (e.g.
    /// [`TurApp::with_element`](crate::TurApp::with_element)).
    pub(crate) fn worker_tx(&self) -> &WorkerTx {
        &self.worker_tx
    }

    /// The single worker→main message handler. Pure dispatch + side-effects
    /// for rendering policy: `RenderCommands` is handed back as
    /// [`MsgOutcome::Render`] so `run_loop` can buffer it for vsync-aligned
    /// pipelining. All backend mutations (`cursor_backend`,
    /// `focus_changed_handler`, image uploads, event-bus dispatch) happen
    /// here.
    pub(crate) fn apply_msg(&self, msg: MainMsg) -> MsgOutcome {
        match msg {
            MainMsg::RenderCommands { commands } => MsgOutcome::Render(commands),
            MainMsg::UploadImage { id, image } => {
                // Retain the full resource (pixel Blob) on main for
                // context-loss re-upload, then upload into the GPU atlas.
                self.insert_image_resource(id, image.clone());
                self.upload_image_resource(id, &image);
                MsgOutcome::Continue
            }
            MainMsg::CursorChanged(cursor) => {
                #[allow(clippy::collapsible_if)]
                if let Some(backend) = self.cursor_backend.borrow().as_ref() {
                    if let Ok(mut b) = backend.lock() {
                        b.set_cursor(cursor);
                    }
                }
                MsgOutcome::Continue
            }
            MainMsg::FocusedStateChanged {
                is_editable,
                cursor_rect,
            } => {
                // Clone the Rc out before invoking so a handler that
                // re-enters the backend (or calls a method borrowing the
                // RefCell) can't panic on a double borrow.
                let handler = self.focus_changed_handler.borrow().clone();
                if let Some(handler) = handler {
                    handler(FocusedState {
                        is_editable,
                        cursor_rect,
                    });
                }
                MsgOutcome::Continue
            }
            MainMsg::FrameOutcome(Ok(outcome)) => MsgOutcome::Frame(outcome),
            MainMsg::FrameOutcome(Err(e)) => MsgOutcome::Failed(e),
            MainMsg::Destroyed => MsgOutcome::Closed,
            MainMsg::EventBusToHost(bytes) => {
                self.event_bus_handle.dispatch_to_host(bytes);
                MsgOutcome::Continue
            }
            // DevReply — handlers ignore (the Reply<T> slot handles RPC replies).
            _ => MsgOutcome::Continue,
        }
    }

    /// RPC dispatch — send a [`WorkerMsg`] with a Reply slot, await the
    /// reply. Async: the caller (e.g. `eval_js`) is itself `async fn`;
    /// the embedder drives it via its runtime.
    pub(crate) async fn rpc<T: 'static>(
        &self,
        msg_builder: impl FnOnce(crate::core::app::ReplySender<T>) -> WorkerMsg,
    ) -> T {
        let (tx, rx) = Reply::<T>::pair();
        let msg = msg_builder(tx);
        let _ = self.worker_tx.unbounded_send(msg);
        self.wake_worker();
        rx.rx.await.expect("reply sender dropped without firing")
    }

    pub async fn load_js(&self, source: &str) -> Result<(), ModuleError> {
        self.rpc(|tx| WorkerMsg::LoadJs {
            source: std::sync::Arc::from(source),
            reply: tx,
        })
        .await
    }

    pub async fn load_module(&self, source: &str) -> Result<(), ModuleError> {
        self.rpc(|tx| WorkerMsg::LoadModule {
            source: std::sync::Arc::from(source),
            reply: tx,
        })
        .await
    }

    pub async fn eval_module(&self, source: &str) -> Result<(), ModuleError> {
        self.rpc(|tx| WorkerMsg::EvalModule {
            source: std::sync::Arc::from(source),
            reply: tx,
        })
        .await
    }

    /// Synchronous JS expression evaluation. Test-only — production code
    /// uses `load_module` / `eval_module`. Useful for inspecting JS-side
    /// state via `globalThis.__x = ...`.
    pub async fn eval_js(&self, source: &str) -> String {
        self.rpc(|tx| WorkerMsg::EvalJs {
            source: std::sync::Arc::from(source),
            reply: tx,
        })
        .await
    }

    pub async fn focused_state(&self) -> FocusedState {
        self.rpc(|tx| WorkerMsg::QueryFocusedState { reply: tx })
            .await
    }

    pub async fn focused_element(&self) -> Option<ElementNodeId> {
        self.rpc(|tx| WorkerMsg::QueryFocusedElement { reply: tx })
            .await
    }

    pub async fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        self.rpc(|tx| WorkerMsg::QueryFocusedCursorRect { reply: tx })
            .await
    }

    pub async fn focused_is_editable(&self) -> bool {
        self.rpc(|tx| WorkerMsg::QueryFocusedIsEditable { reply: tx })
            .await
    }

    pub async fn query_element(&self, key: &[&str]) -> Option<NodeId> {
        let key_owned: Vec<String> = key.iter().map(|s| s.to_string()).collect();
        self.rpc(|tx| WorkerMsg::QueryElement {
            key: key_owned,
            reply: tx,
        })
        .await
    }

    pub async fn dev_tool_element_tree(&self) -> Option<DevNodeData> {
        self.rpc(|tx| WorkerMsg::DevElementTree { reply: tx }).await
    }

    pub async fn dev_tool_get_element(&self, id: NodeId) -> Option<DevNodeData> {
        self.rpc(|tx| WorkerMsg::DevGetElement { id, reply: tx })
            .await
    }

    /// Test/dev-tool RPC: full element-tree snapshot.
    pub async fn query_tree_snapshot(&self) -> NodeTreeSnapshot {
        self.rpc(|tx| WorkerMsg::QueryTreeSnapshot { reply: tx })
            .await
    }

    /// Count of image resources retained on main (pixel `Blob`s). Test-only
    /// introspection: asserts `MainMsg::UploadImage` was received (shipped
    /// directly from the `createImageResource` bridge) and inserted into
    /// main's `ImageResourceMap`.
    #[doc(hidden)]
    pub fn image_resource_count(&self) -> usize {
        self.image_resource_map.borrow().iter_images().count()
    }
}

/// Worker thread loop. Runs as `async fn` driven by the platform's
/// `spawn_worker` (native: `LocalPool::run_until`; wasm: `spawn_local`).
///
/// Awaits on `worker_rx.recv()` for incoming `WorkerMsg`s. On `Wake`,
/// pumps the engine (`backend.pump()`), then ships:
/// 1. `MainMsg::RenderCommands` (if the flush painted)
/// 2. `MainMsg::FrameOutcome` (always)
/// 3. `MainMsg::CursorChanged` (deduped against `last_cursor`)
/// 4. `MainMsg::FocusedStateChanged` (deduped against `last_focus`)
///
/// `MainMsg::UploadImage` is **not** shipped here — decoded images are
/// shipped directly from the `createImageResource` bridge via the shared
/// `main_tx` clone held in `TurJsContext` (one ship per decode, FIFO).
/// `MainMsg::Resized` is also not shipped — the embedder resizes the
/// main-side renderer directly at event-receipt time and forwards
/// `PlatformEvent::Resize` here for layout.
///
/// All other variants (`PlatformEvent`, `RequestPaint`, RPCs) are
/// dispatched to `backend.handle_worker_msg` (RPC variants fire their own
/// `ReplySender`).
async fn worker_loop(backend: WorkerBackend, mut worker_rx: WorkerRx, main_tx: MainTx) {
    let mut last_cursor: Option<crate::core::platform::Cursor> = None;
    type FocusCache = Option<(bool, Option<(f64, f64, f64, f64)>)>;
    let mut last_focus: FocusCache = None;
    while let Some(msg) = worker_rx.next().await {
        match msg {
            WorkerMsg::Wake => {
                let outcome = backend.pump();
                let payload = match outcome {
                    Ok(fo) => Ok(fo),
                    Err(e) => {
                        tracing::error!("worker pump error: {e}");
                        Err(e.to_string())
                    }
                };
                // Ship render commands if the flush painted.
                if let Some(batch) = backend.take_pending_render_batch() {
                    let _ = main_tx.unbounded_send(MainMsg::RenderCommands { commands: batch });
                }
                let _ = main_tx.unbounded_send(MainMsg::FrameOutcome(payload));
                // Ship cursor changes (deduped against the last emitted).
                let current_cursor = backend.last_applied_cursor();
                if current_cursor != last_cursor {
                    last_cursor = current_cursor;
                    let _ = main_tx
                        .unbounded_send(MainMsg::CursorChanged(current_cursor.unwrap_or_default()));
                }
                // Ship focus-state changes (deduped against the last
                // emitted) — `apply_msg` forwards them to an embedder-
                // installed handler (no engine-side cache).
                let current_focus = backend.focused_state();
                let focus_key = (current_focus.is_editable, current_focus.cursor_rect);
                if Some(focus_key) != last_focus {
                    last_focus = Some(focus_key);
                    let _ = main_tx.unbounded_send(MainMsg::FocusedStateChanged {
                        is_editable: current_focus.is_editable,
                        cursor_rect: current_focus.cursor_rect,
                    });
                }
            }
            WorkerMsg::Destroy { reply } => {
                let _ = main_tx.unbounded_send(MainMsg::Destroyed);
                reply.send(());
                break;
            }
            // All other variants (PlatformEvent, RequestPaint, LoadModule,
            // Dev*, EventBusToJs) delegate to the worker dispatch — RPC
            // variants fire their own ReplySender.
            other => backend.handle_worker_msg(other),
        }
    }
}
