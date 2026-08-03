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
//!   replies. The embedder wires a `render_sink` callback that receives
//!   each `MainMsg::RenderCommands` batch + image map + viewport, and
//!   applies it to the main-side renderer.
//!
//! ## Async model
//!
//! All channels use `futures::channel` (mpsc + oneshot). The worker thread entry-point wraps
//! an `async fn worker_loop(...)` via `futures::executor::block_on`, so the
//! worker awaits on `worker_rx.recv()` instead of blocking on a Mutex +
//! Condvar. Main-thread `pump` and `rpc` are `async fn`; the embedder
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
use crate::core::image_resource::ImageResourceMap;
use crate::core::platform::CursorBackend;
use crate::core::render::RenderCommand;
use crate::core::thread::{Builder as ThreadBuilder, JoinHandle as ThreadJoinHandle};
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

    /// Snapshot the worker-side `ImageResourceMap` for shipping to main.
    pub(crate) fn image_resource_map_snapshot(&self) -> std::sync::Arc<ImageResourceMap> {
        self.internal.image_resource_map_snapshot()
    }

    pub(crate) fn screen_viewport(&self) -> (u32, u32, f64) {
        let cx = self.internal.app_context.borrow();
        let (w, h) = cx.screen.logical_size;
        (w as u32, h as u32, cx.screen.dpr)
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
            WorkerMsg::RequestPaint => {
                self.internal.js_context.need_paint.set(true);
            }
            WorkerMsg::Wake => {
                // The worker_loop drives flush via `pump()` (separate method
                // so it can capture the FrameOutcome + ship commands).
            }
            WorkerMsg::LoadModule { source, reply } => {
                let res = self.load_module_inner(&source);
                reply.send(res);
            }
            WorkerMsg::LoadJs { source, reply } => {
                let res = self.load_js_inner(&source);
                reply.send(res);
            }
            WorkerMsg::EvalModule { source, reply } => {
                let res = self.eval_module_inner(&source);
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
            WorkerMsg::EventBusToJs(_bytes) => {
                tracing::trace!(
                    "EventBusToJs: {} bytes (not yet wired for delivery)",
                    _bytes.len()
                );
            }
            WorkerMsg::AppEvent(event) => {
                self.push_app_event(event);
            }
            WorkerMsg::RenderToPixels { reply } => {
                // No renderer on the worker — main owns it. Always None.
                let _ = reply;
                tracing::warn!(
                    "WorkerBackend::handle_worker_msg: RenderToPixels is a no-op (renderer lives on main)"
                );
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
// MainBackend — TurApp's backend. Owns the worker + RPC plumbing + render sink
// ---------------------------------------------------------------------------

/// Render sink signature: receives a command batch + image map snapshot +
/// viewport tuple, applies them to the main-side renderer. The embedder
/// (tur-android / tur-wasm) installs this via [`MainBackend::set_render_sink`]
/// after construction.
pub type RenderSink = Box<dyn FnMut(&[RenderCommand], &ImageResourceMap, (u32, u32, f64))>;

/// The public backend owned by `TurApp`. Spawns a worker thread running a
/// [`WorkerBackend`], dispatches input via `futures::channel`, and receives
/// [`MainMsg`] replies.
///
/// ## Async pump / rpc
///
/// All public methods on `MainBackend` are `async fn`. The embedder
/// supplies the runtime — `wasm_bindgen_futures::spawn_local` on wasm
/// (so the JS main thread never blocks), `futures::executor::block_on`
/// on native (so the calling thread parks until the worker replies).
///
/// ## Cross-thread render shipping
///
/// After each `pump()`, the worker_loop drains the worker's
/// `pending_render_batch` and ships it via `MainMsg::RenderCommands { commands,
/// image_map, viewport }`. `MainBackend::pump` matches that variant and
/// invokes the [`render_sink`](Self::set_render_sink) callback, which applies
/// the batch to the main-side renderer.
///
/// ## Cached cursor / focus state
///
/// The worker emits `MainMsg::CursorChanged` and `MainMsg::FocusedStateChanged`
/// (deduped against the previous frame) alongside the FrameOutcome. Main
/// caches the latest values in `cached_cursor` / `cached_focus`, available
/// for non-blocking reads from embedder callbacks (e.g. the wasm
/// after-frame hook reads focus state without an RPC).
///
/// `RenderToPixels` is no-op on the worker (the renderer lives on main);
/// screenshot tests should run with main-side render access instead.
pub struct MainBackend {
    worker_tx: WorkerTx,
    /// Wrapped in `RefCell` because `futures::channel::mpsc::UnboundedReceiver::next`
    /// requires `&mut self`, but `MainBackend` is held inside `Rc<TurApp>`
    /// on wasm + android (single-threaded ownership). The borrow is held
    /// across the `next().await` in `pump` — safe because the wasm main
    /// thread is single-threaded and `Rc<TurApp>` itself enforces
    /// single-threaded access.
    main_rx: RefCell<MainRx>,
    /// Holds the worker `JoinHandle` alive for the backend's lifetime so
    /// the worker thread (or Web Worker on wasm) doesn't get reclaimed.
    _worker_handle: ThreadJoinHandle<()>,
    /// Main-side cursor backend. Worker emits `MainMsg::CursorChanged` on
    /// cursor state change; main applies it here during `pump`. Set via
    /// `set_cursor_backend` (called by embedder after `create_app`).
    cursor_backend: RefCell<Option<Arc<std::sync::Mutex<dyn CursorBackend + Send + Sync>>>>,
    /// Latest cursor received from the worker (cached for non-blocking
    /// reads from embedder callbacks).
    cached_cursor: RefCell<crate::core::platform::Cursor>,
    /// Latest focus state received from the worker (cached for
    /// non-blocking reads — e.g. wasm's after-frame hook reads focus
    /// without an RPC).
    cached_focus: RefCell<FocusedState>,
    /// Cross-thread event bus handle. Routes `emit_to_js` via
    /// `WorkerMsg::EventBusToJs` (channel mode).
    event_bus_handle: crate::core::event_bus::EventBusHandle,
    /// Main-side render sink. Worker emits `MainMsg::RenderCommands` after
    /// each paint; main invokes the sink to drive its renderer.
    render_sink: RefCell<Option<RenderSink>>,
}

impl MainBackend {
    /// Spawn a worker thread that owns a [`WorkerBackend`] produced by
    /// `backend_factory`. The factory runs on the worker thread (so it can
    /// construct `!Send` types like `Rc<dyn Clock>` and `boa::Context`).
    ///
    /// The factory must be `Send + 'static` — capture only `Send` config
    /// (plugin vecs, capability factories, etc.), not `Rc`/`RefCell` state.
    ///
    /// The worker thread's entry-point wraps `worker_loop` (an `async fn`)
    /// in `futures::executor::block_on`. On wasm this is a real Web Worker
    /// (via `wasm_thread`), where `block_on` + `Atomics.wait` are allowed
    /// (workers can block; the main thread cannot).
    pub(crate) fn new(backend_factory: impl FnOnce() -> WorkerBackend + Send + 'static) -> Self {
        let (worker_tx, worker_rx) = futures::channel::mpsc::unbounded::<WorkerMsg>();
        let (main_tx, main_rx) = futures::channel::mpsc::unbounded::<MainMsg>();

        // One-shot init signal: worker fires after `backend_factory()` (which
        // runs `plugin.register` + capability replay) completes. Native main
        // blocks on this so `create_app` returning guarantees the worker's
        // plugin-level side effects are observable. On wasm the main thread
        // cannot block — embedders must await an RPC instead.
        #[cfg(not(target_arch = "wasm32"))]
        let (init_tx, init_rx) = std::sync::mpsc::channel::<()>();

        let worker_handle = ThreadBuilder::new()
            .name("tur-worker".into())
            .spawn(move || {
                let backend = backend_factory();
                #[cfg(not(target_arch = "wasm32"))]
                let _ = init_tx.send(());
                // Drive the async worker_loop from this thread. block_on
                // parks the worker thread when awaiting on
                // `worker_rx.next()`; futures::channel's waker unparks it when
                // main sends a message.
                futures::executor::block_on(worker_loop(backend, worker_rx, main_tx));
            })
            .expect("failed to spawn tur worker thread");

        // Native: synchronously wait for the worker to finish init.
        #[cfg(not(target_arch = "wasm32"))]
        init_rx
            .recv()
            .expect("worker thread died during backend_factory");

        Self {
            worker_tx: worker_tx.clone(),
            main_rx: RefCell::new(main_rx),
            _worker_handle: worker_handle,
            cursor_backend: RefCell::new(None),
            cached_cursor: RefCell::new(crate::core::platform::Cursor::default()),
            cached_focus: RefCell::new(FocusedState::default()),
            event_bus_handle: crate::core::event_bus::EventBusHandle::from_channel(worker_tx),
            render_sink: RefCell::new(None),
        }
    }

    /// Install the main-side render sink. Called by the embedder after
    /// `TurRuntime::create_app` (the worker ships commands; main renders).
    pub fn set_render_sink<
        F: FnMut(&[RenderCommand], &ImageResourceMap, (u32, u32, f64)) + 'static,
    >(
        &self,
        f: F,
    ) {
        *self.render_sink.borrow_mut() = Some(Box::new(f));
    }

    /// Install the main-side cursor backend. Worker emits
    /// `MainMsg::CursorChanged`; main applies here during `pump`.
    pub fn set_cursor_backend(
        &self,
        backend: Arc<std::sync::Mutex<dyn CursorBackend + Send + Sync>>,
    ) {
        *self.cursor_backend.borrow_mut() = Some(backend);
    }

    /// Cross-thread event bus handle (queues mode is unused; the worker's
    /// `EventBus` isn't reachable from main).
    pub fn event_bus_handle(&self) -> crate::core::event_bus::EventBusHandle {
        self.event_bus_handle.clone()
    }

    /// Send a fire-and-forget [`WorkerMsg`] (no Reply slot). Used by
    /// `push_platform_event`, `request_paint`, `push_app_event`,
    /// `emit_to_js`.
    pub(crate) fn send_worker_msg(&self, msg: WorkerMsg) {
        let _ = self.worker_tx.unbounded_send(msg);
    }

    /// Borrow the worker→main channel sender. Used by call sites that
    /// build a `WorkerMsg` carrying a closure / reply slot directly (e.g.
    /// [`TurApp::with_element`](crate::TurApp::with_element)).
    pub(crate) fn worker_tx(&self) -> &WorkerTx {
        &self.worker_tx
    }

    /// Advance one frame: send `Wake` to the worker, await the next
    /// `MainMsg::FrameOutcome`. Any `RenderCommands`, `CursorChanged`,
    /// or `FocusedStateChanged` arriving in the meantime are dispatched
    /// to the render sink / cursor backend / focus cache respectively.
    ///
    /// Async: the embedder drives this via its platform's runtime
    /// (`wasm_bindgen_futures::spawn_local` on wasm;
    /// `futures::executor::block_on` on native). The wasm main thread
    /// never blocks — the future suspends on `main_rx.next().await` and
    /// resumes when the worker posts data.
    /// `pump` holds a `RefCell<MainRx>::borrow_mut()` across `next().await`.
    /// Clippy flags this as `await_holding_refcell_ref`, but it's safe:
    /// `Rc<TurApp>` enforces single-threaded access on wasm + android, and
    /// pump is driven sequentially from the rAF loop (the engine-side
    /// `pump_in_progress` guard rejects overlap).
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn pump(&self) -> Result<FrameOutcome, TurError> {
        self.worker_tx
            .unbounded_send(WorkerMsg::Wake)
            .map_err(|_| TurError::Other("worker gone".into()))?;
        let mut rx = self.main_rx.borrow_mut();
        loop {
            match rx.next().await {
                Some(MainMsg::RenderCommands {
                    commands,
                    image_map,
                    viewport,
                }) => {
                    if let Some(sink) = self.render_sink.borrow_mut().as_mut() {
                        sink(&commands, &image_map, viewport);
                    }
                }
                Some(MainMsg::FrameOutcome(result)) => {
                    return result.map_err(TurError::Other);
                }
                Some(MainMsg::CursorChanged(cursor)) => {
                    *self.cached_cursor.borrow_mut() = cursor;
                    #[allow(clippy::collapsible_if)]
                    if let Some(backend) = self.cursor_backend.borrow().as_ref() {
                        if let Ok(mut b) = backend.lock() {
                            b.set_cursor(cursor);
                        }
                    }
                }
                Some(MainMsg::FocusedStateChanged {
                    is_editable,
                    cursor_rect,
                }) => {
                    *self.cached_focus.borrow_mut() = FocusedState {
                        is_editable,
                        cursor_rect,
                    };
                }
                Some(MainMsg::Destroyed) => {
                    return Err(TurError::Other("worker destroyed".into()));
                }
                // EventBusToHost / DevReply — pump ignores for now (the
                // Reply<T> slot handles RPC replies; standalone MainMsg
                // variants are reserved for future event-bus work).
                Some(_) => continue,
                None => return Err(TurError::Other("worker gone".into())),
            }
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

    /// Read the latest cursor received from the worker (non-blocking).
    /// Updated during `pump` when `MainMsg::CursorChanged` arrives.
    pub fn cached_cursor(&self) -> crate::core::platform::Cursor {
        *self.cached_cursor.borrow()
    }

    /// Read the latest focus state received from the worker (non-blocking).
    /// Updated during `pump` when `MainMsg::FocusedStateChanged` arrives.
    pub fn cached_focus(&self) -> FocusedState {
        self.cached_focus.borrow().clone()
    }
}

/// Worker thread loop. Runs as `async fn` driven by
/// `futures::executor::block_on` in the worker thread entry-point.
///
/// Awaits on `worker_rx.recv()` for incoming `WorkerMsg`s. On `Wake`,
/// pumps the engine (`backend.pump()`), then ships:
/// 1. `MainMsg::RenderCommands` (if the flush painted)
/// 2. `MainMsg::FrameOutcome` (always)
/// 3. `MainMsg::CursorChanged` (deduped against `last_cursor`)
/// 4. `MainMsg::FocusedStateChanged` (deduped against `last_focus`)
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
                    let image_map = backend.image_resource_map_snapshot();
                    let viewport = backend.screen_viewport();
                    let _ = main_tx.unbounded_send(MainMsg::RenderCommands {
                        commands: batch,
                        image_map,
                        viewport,
                    });
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
                // emitted) — main caches it for non-blocking reads from
                // embedder callbacks (e.g. wasm's after-frame hook).
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
