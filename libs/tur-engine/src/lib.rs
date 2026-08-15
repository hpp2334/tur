pub mod builtin_plugins;
pub mod core;
pub mod renderer;

pub mod error;

// Re-export engine-internal cursor-capability types at the crate root so
// embedders can write `tur_engine::CursorBackend`, `tur_engine::CursorCap`,
// `tur_engine::NoopCursor` without reaching into `core::platform`. The std
// plugin itself (`TurStdPlugin`) lives in `builtin_plugins::std`.
pub use crate::core::platform::{CursorBackend, CursorCap, NoopCursor};
// Re-export the clipboard plugin surface at the crate root so embedders /
// external backend crates can write `tur_engine::Clipboard`,
// `tur_engine::ClipboardBackend`, `tur_engine::TurClipboardPlugin`,
// `tur_engine::platform_paste`. The plugin itself lives in
// `builtin_plugins::clipboard` (inlined from the former
// `tur-clipboard-capability` crate).
pub use crate::builtin_plugins::clipboard::{
    Clipboard, ClipboardBackend, TurClipboardPlugin, platform_paste,
};
// Re-export `TurStdPlugin` at the crate root so embedders can write
// `tur_engine::TurStdPlugin` (was previously in a separate `tur-std` crate).
pub use crate::builtin_plugins::TurStdPlugin;
pub use crate::core::event_bus::EventBus;
// Re-export the runtime + builder at the crate root — the primary entry point
// for embedders. `TurRuntime::builder()` is the shared, created-once object;
// `runtime.app_builder().renderer(r, viewport, dpr).build()` spawns an isolated
// `TurApp` instance (engine on a worker thread; `MainBackend` owns the renderer
// on main and drives it directly — no render_sink callback).
pub use crate::core::runtime::{MainBackend, TurAppBuilder, TurRuntime, TurRuntimeBuilder};
// Re-export the plugin-layer main-thread hop surface so backends in other
// crates (`tur-clipboard-native`, future OS-API backends) can name the type
// without reaching into `core::plugin`. OS-API backends receive an
// `AsyncPluginContext` clone at construction (via the closure form of
// `TurRuntimeBuilder::capability`) and hop OS-API calls onto the engine's
// main thread (required by some platforms — e.g. macOS
// `arboard`/`NSPasteboard`); plugin code reaches the same channel via
// `PluginContext::to_async`. The engine creates the channel internally at
// `build()`, so no embedder wiring is required.
pub use crate::core::plugin::{AsyncPluginContext, MainRunFuture};
pub use crate::core::scheduler::SpawnError;
// Re-export the worker-pool declaration type so embedders can write
// `tur_engine::WorkerPoolHandle` (registered via
// `TurRuntimeBuilder::worker_pool`, assigned via `TurAppBuilder::worker_pool`).
pub use crate::core::scheduler::WorkerPoolHandle;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use error::TurError;

use core::app::FrameOutcome;

/// Snapshot of focused-element state — single struct for the two-value
/// `focused_is_editable` + `focused_cursor_rect` pair. Delivered to
/// embedders via [`TurApp::set_focus_changed_handler`] (push, on change)
/// and [`TurApp::focused_state`] (async RPC, on demand).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FocusedState {
    pub is_editable: bool,
    /// Logical-space `(x, y, w, h)` of the focused element's caret, or
    /// `None` if no editable is focused.
    pub cursor_rect: Option<(f64, f64, f64, f64)>,
}

/// A running tur engine instance.
///
/// Wraps a [`MainBackend`] that owns a worker thread (running a
/// [`WorkerBackend`](core::runtime::WorkerBackend)) **and** the main-side
/// renderer (passed to `TurRuntime::app_builder().build(...)`). Main drives
/// the renderer directly from [`MainBackend`] — no `render_sink` callback.
/// Everything else (boa `Context`, element tree, reactive store, layout,
/// subsystems) lives on the worker.
///
/// Construct via [`TurRuntime::app_builder`] then
/// [`TurAppBuilder::build`](core::runtime::TurAppBuilder::build).
///
/// ## Async API
///
/// All public methods are `async fn`. The embedder supplies the runtime:
/// - On wasm, `wasm_bindgen_futures::spawn_local` runs futures
///   cooperatively on the JS event loop (main thread never blocks).
/// - On native (Android JNI, integration tests), `futures::executor::block_on`
///   parks the calling thread until the future resolves.
pub struct TurApp {
    backend: MainBackend,
    /// Platform main-thread scheduler (vsync events, request_vsync,
    /// spawn_local). Cloned from the runtime at construction; embedders
    /// with per-instance scheduling (Android) replace it via
    /// [`Self::set_main_scheduler`].
    main_sched: RefCell<core::scheduler::MainScheduler>,
    /// Embedder-installed callback fired after each autonomous frame —
    /// typically used for DOM side-effects (file-pick resolution, textarea
    /// focus / caret positioning). `None` in tests.
    after_frame: RefCell<Option<AfterFrameHook>>,
    /// Set by [`Self::run_loop`] — guards against double-spawn of the
    /// autonomous loop.
    loop_started: Cell<bool>,
    /// Set by [`Self::destroy`]. Subsequent wake attempts short-circuit.
    destroyed: Cell<bool>,
}

/// Per-frame hook fired at the end of each iteration of `TurApp::run_loop`
/// (after the frame's render/schedule side-effects are applied). See
/// [`TurApp::set_after_frame_hook`].
pub type AfterFrameHook = Rc<dyn Fn(FrameOutcome)>;

/// Per-focus-change hook fired from [`MainBackend::apply_msg`](core::runtime::MainBackend::apply_msg)
/// when the worker ships a deduped `MainMsg::FocusedStateChanged`. See
/// [`TurApp::set_focus_changed_handler`].
pub type FocusChangedHook = Rc<dyn Fn(FocusedState)>;

impl TurApp {
    /// Construct a `TurApp` backed by the given [`MainBackend`] + scheduler.
    /// The runtime calls this from
    /// [`TurRuntime::app_builder`](crate::core::runtime::TurRuntime::app_builder)
    /// → [`TurAppBuilder::build`](crate::core::runtime::TurAppBuilder::build);
    /// embedders normally don't call it directly.
    pub fn new(backend: MainBackend, main_sched: core::scheduler::MainScheduler) -> Self {
        Self {
            backend,
            main_sched: RefCell::new(main_sched),
            after_frame: RefCell::new(None),
            loop_started: Cell::new(false),
            destroyed: Cell::new(false),
        }
    }

    /// Replace the main-thread scheduler. Used by embedders that need a
    /// per-instance scheduler (e.g. Android, where each instance has its
    /// own JNI `FrameLoop`). Call after `runtime.app_builder().build(...)`
    /// and before `run_loop()`.
    pub fn set_main_scheduler(&self, sched: core::scheduler::MainScheduler) {
        *self.main_sched.borrow_mut() = sched;
    }

    /// Direct accessor on the underlying [`MainBackend`]. Embedders use it
    /// to install the cursor backend after construction.
    pub fn backend(&self) -> &MainBackend {
        &self.backend
    }

    pub async fn load_js(&self, source: &str) -> Result<(), TurError> {
        tracing::info!("load_js: evaluating bundle ({} bytes)", source.len());
        self.backend.load_js(source).await.map_err(TurError::from)
    }

    pub async fn load_module(&self, source: &str) -> Result<(), TurError> {
        tracing::info!("load_module: evaluating module ({} bytes)", source.len());
        self.backend
            .load_module(source)
            .await
            .map_err(TurError::from)
    }

    pub async fn eval_module(&self, source: &str) -> Result<(), TurError> {
        self.backend
            .eval_module(source)
            .await
            .map_err(TurError::from)
    }

    /// Read rendered pixels back from the owned renderer (screenshot
    /// tests). Returns `None` if the renderer doesn't support readback.
    pub fn render_to_pixels(&self) -> Option<Vec<u8>> {
        self.backend.render_to_pixels()
    }

    /// Cross-thread-safe event bus handle. `emit_to_js` ships via the
    /// worker's channel; `drain_js_to_host` returns empty on main (the
    /// worker emits `MainMsg::EventBusToHost` separately when needed).
    pub fn event_bus_handle(&self) -> core::event_bus::EventBusHandle {
        self.backend.event_bus_handle()
    }

    /// Combined focused-element state — RPC variant (awaits the worker's
    /// reply). For change-driven reads, register
    /// [`Self::set_focus_changed_handler`] (push, fired on focus / caret
    /// change).
    pub async fn focused_state(&self) -> FocusedState {
        self.backend.focused_state().await
    }

    /// Push a platform (input) event from the embedder — resize, pointer,
    /// wheel, key, IME, or paste. Re-arms an idle autonomous loop.
    pub fn push_platform_event(&self, event: core::platform::PlatformEvent) {
        self.backend
            .send_worker_msg(core::app::WorkerMsg::PlatformEvent(event));
        self.request_wakeup();
    }

    /// Resize the surface. The embedder calls this at resize-event-receipt
    /// time (DOM `ResizeObserver` / winit / JNI): it resizes the main-side
    /// renderer directly (no flush + worker→main round-trip — lower
    /// latency) AND forwards `PlatformEvent::Resize` to the worker so
    /// `ResizeSubsystem` updates `Screen` / `viewportSize$` for layout.
    /// Event-driven, not per-frame, so no dedup is needed.
    pub fn resize(&self, logical_width: u32, logical_height: u32, dpr: f64) {
        self.backend.resize(logical_width, logical_height, dpr);
        self.backend
            .send_worker_msg(core::app::WorkerMsg::PlatformEvent(
                core::platform::PlatformEvent::Resize {
                    logical_width,
                    logical_height,
                    dpr,
                },
            ));
        self.request_wakeup();
    }

    /// Push an engine-internal event onto the app-event bus (programmatic
    /// scrolls, clipboard writes). Re-arms an idle autonomous loop.
    pub fn push_app_event(&self, event: core::app::AppEvent) {
        self.backend
            .send_worker_msg(core::app::WorkerMsg::AppEvent(event));
        self.request_wakeup();
    }

    /// The autonomous frame loop — driven by the embedder's platform loop
    /// (Choreographer-polled on Android, `spawn_local`'d on wasm). The
    /// engine owns all frame logic; the platform only supplies the wake-up
    /// cadence via [`MainScheduler::vsync_events`](core::scheduler::MainScheduler::vsync_events)
    /// + [`MainScheduler::request_vsync`](core::scheduler::MainScheduler::request_vsync).
    ///
    /// Each iteration races the platform vsync stream against the worker's
    /// `MainMsg` stream:
    /// - **vsync** — kick the worker for the NEXT frame (it flushes+records
    ///   N+1 while main encodes N), then paint the latest buffered batch
    ///   (vsync-aligned, latest-wins).
    /// - **worker msg** — dispatch via [`MainBackend::apply_msg`](core::runtime::MainBackend::apply_msg),
    ///   the single shared handler. `RenderCommands` is buffered into
    ///   `pending` for vsync-aligned pipelining; `FrameOutcome` fires the
    ///   `after_frame` hook and re-arms vsync (or flushes `pending` on
    ///   quiescence). Side-effects (cursor, focus-change handler, image
    ///   uploads) are applied inside `apply_msg`.
    ///
    /// The bootstrap is automatic: `app_builder().build(...)` pushes an
    /// initial resize event to the worker, the worker pumps + ships
    /// `FrameOutcome` back, and the loop requests the next vsync based on
    /// the outcome. No initial `request_vsync()` is needed.
    ///
    /// Concurrency: single-loop serialized. The embedder must spawn this
    /// future exactly once per `TurApp`. Multiple concurrent calls panic.
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn run_loop(self: Rc<Self>) {
        assert!(
            !self.loop_started.replace(true),
            "run_loop called twice on the same TurApp"
        );

        let mut vsync_rx = self.main_sched.borrow().vsync_events();
        // `main_rx` is in a RefCell; borrow for the lifetime of this loop.
        // Safe: run_loop is called exactly once per app (asserted above).
        #[allow(clippy::await_holding_refcell_ref)]
        let mut main_rx = self.backend.main_rx.borrow_mut();

        use crate::core::runtime::MsgOutcome;
        use futures::future::{Either, select};
        use futures::stream::StreamExt;

        // Pipelining buffer: the latest un-rendered batch from the worker.
        let mut pending: Option<core::render::RenderCommandBatch> = None;

        loop {
            // Race vsync + main_msg streams — first to fire wins.
            let vsync_fut = vsync_rx.next();
            let main_fut = main_rx.next();
            let event = select(vsync_fut, main_fut).await;

            match event {
                Either::Left((Some(()), _)) => {
                    if self.destroyed.get() {
                        break;
                    }
                    // 1) Kick the worker for the NEXT frame first — it
                    //    flushes+records N+1 while main encodes N below.
                    self.backend.send_worker_msg(core::app::WorkerMsg::Wake);
                    // 2) Render the latest buffered batch (vsync-aligned,
                    //    latest-wins). Skip empty batches — an empty command
                    //    list paints a blank frame (clears the surface), which
                    //    is never desirable.
                    if let Some(batch) = pending.take().filter(|b| !b.is_empty()) {
                        self.backend.render_batch(&batch);
                    }
                }
                Either::Left((None, _)) => break,
                Either::Right((Some(msg), _)) => {
                    let stop = match self.backend.apply_msg(msg) {
                        MsgOutcome::Render(batch) => {
                            // Pipelined: buffer (latest-wins); rendered at
                            // the next vsync.
                            pending = Some(batch);
                            false
                        }
                        MsgOutcome::Frame(outcome) => {
                            // Apply this frame's render/schedule side-effects
                            // BEFORE firing the after_frame hook, so hook
                            // observers (test harnesses reading pixels/state,
                            // embedders syncing DOM) see the fully-applied
                            // frame. The hook is the last thing the loop
                            // does for this message.
                            let stop = if outcome.schedule == core::app::NextFrame::Vsync {
                                self.main_sched.borrow().request_vsync();
                                false
                            } else if let Some(batch) = pending.take().filter(|b| !b.is_empty()) {
                                // Quiescence: no vsync is armed (nothing
                                // time-driven pending), so the pipeline
                                // would stall with an un-rendered batch
                                // (e.g. the initial frame, or a one-shot
                                // paint request). Flush it now (empty
                                // batches skipped — they'd paint blank) —
                                // the next frame only starts on a new input.
                                self.backend.render_batch(&batch);
                                false
                            } else {
                                // Idle + empty pending: no-op. The loop
                                // blocks on the next event.
                                false
                            };
                            if let Some(hook) = self.after_frame.borrow().as_ref().cloned() {
                                hook(outcome);
                            }
                            stop
                        }
                        MsgOutcome::Failed(e) => {
                            tracing::error!("worker frame error: {e}");
                            false
                        }
                        MsgOutcome::Closed => true,
                        MsgOutcome::Continue => false,
                    };
                    if stop {
                        break;
                    }
                }
                Either::Right((None, _)) => break,
            }
        }
    }

    /// Install a callback fired at the end of each `run_loop` iteration,
    /// after the frame's render/schedule side-effects are applied.
    pub fn set_after_frame_hook(&self, hook: Option<Rc<dyn Fn(FrameOutcome)>>) {
        *self.after_frame.borrow_mut() = hook;
    }

    /// Mark the app as destroyed. Subsequent `wake` attempts short-circuit.
    /// Sends `WorkerMsg::Destroy` to drain the worker.
    pub fn destroy(&self) {
        self.destroyed.set(true);
        // Fire-and-forget — the worker drains and exits. We don't await
        // the reply (would block on a sync API).
        let (tx, _rx) = core::app::Reply::<()>::pair();
        self.backend
            .send_worker_msg(core::app::WorkerMsg::Destroy { reply: tx });
    }

    /// Re-arm an idle autonomous loop: ask the scheduler for one wake-up
    /// on the next frame. Idempotent at the driver (armed flag).
    fn request_wakeup(&self) {
        self.main_sched.borrow().request_vsync();
    }

    pub async fn dev_tool_element_tree(&self) -> Option<core::elements::DevNodeData> {
        self.backend.dev_tool_element_tree().await
    }

    pub async fn dev_tool_get_element(
        &self,
        id: core::element::NodeId,
    ) -> Option<core::elements::DevNodeData> {
        self.backend.dev_tool_get_element(id).await
    }

    /// Run `cb` against the live `AnyElement` at `id`, on the worker
    /// thread. The closure executes where the element actually lives, so
    /// it can do typed introspection that can't be serialized across the
    /// thread boundary (e.g. `e.cast::<TextElement>().spans()`). Returns
    /// `None` if the id is unknown or the node has no element (fragment
    /// host).
    ///
    /// Constraints:
    /// - `R: Send + 'static` — the result crosses the worker→main channel.
    /// - `cb: Send + 'static` — the closure crosses main→worker.
    ///
    /// Production code should never call this — it pins test-only typed
    /// element access to the worker. Use `dev_tool_get_element` /
    /// `query_tree_snapshot` for serializable snapshots.
    pub async fn with_element<R: Send + 'static>(
        &self,
        id: core::element::ElementNodeId,
        cb: impl FnOnce(&core::elements::AnyElement) -> R + Send + 'static,
    ) -> Option<R> {
        use core::app::comm::{Reply, WorkerMsg};
        use core::elements::NodeTreeData;

        let (tx, rx) = Reply::<Option<R>>::pair();
        let runner: Box<dyn FnOnce(&NodeTreeData) + Send + 'static> = Box::new(move |tree| {
            let result = tree
                .get_element(id)
                .and_then(|node| node.element.as_ref())
                .map(cb);
            tx.send(result);
        });
        // Best-effort send — returns Err if the worker is gone. In that case
        // we'll just await on `rx` which yields None.
        let _ = self
            .backend
            .worker_tx()
            .unbounded_send(WorkerMsg::WithElement { id, runner });
        rx.rx.await.unwrap_or(None)
    }

    pub async fn query_element(&self, key: &[&str]) -> Option<core::element::NodeId> {
        self.backend.query_element(key).await
    }

    pub async fn focused_element(&self) -> Option<core::element::ElementNodeId> {
        self.backend.focused_element().await
    }

    pub async fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        self.backend.focused_cursor_rect().await
    }

    /// True if the currently-focused element is an editable text element.
    pub async fn focused_is_editable(&self) -> bool {
        self.backend.focused_is_editable().await
    }

    /// Install a handler fired whenever the worker ships a deduped
    /// `MainMsg::FocusedStateChanged` (i.e. the focused element's
    /// editable-ness or caret rect changes). The engine retains no focus
    /// cache — embedders obtain focus state either by registering here
    /// (push, on change) or via [`Self::focused_state`] (async RPC, on
    /// demand). Pass `None` to clear.
    ///
    /// The handler runs inside [`Self::apply_msg`](core::runtime::MainBackend::apply_msg),
    /// so it observes identical state on the `run_loop` path. Used by the
    /// wasm embedder (textarea focus / caret positioning) and Android
    /// (soft-keyboard sync via a JNI callback into the Kotlin `FrameLoop`).
    pub fn set_focus_changed_handler(&self, handler: Option<FocusChangedHook>) {
        self.backend.set_focus_changed_handler(handler);
    }

    /// Override the main-side cursor backend. The worker emits
    /// `MainMsg::CursorChanged` on cursor state change; main applies here.
    pub fn set_cursor_backend(
        &self,
        backend: std::sync::Arc<std::sync::Mutex<dyn core::platform::CursorBackend + Send + Sync>>,
    ) {
        self.backend.set_cursor_backend(backend);
    }
}
