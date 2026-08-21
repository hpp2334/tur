pub mod builtin_plugins;
pub mod core;
pub mod renderer;

pub mod error;

// Re-export shell-layer types at the crate root so embedders can write
// `tur_engine::Shell`, `tur_engine::NoopShell`, `tur_engine::Cursor`,
// `tur_engine::TextInputState`, `tur_engine::ShellCommand` without reaching
// into `core::shell`. The std plugin itself (`TurStdPlugin`) lives in
// `builtin_plugins::std`.
pub use crate::core::app::ShellCommand;
pub use crate::core::shell::{Cursor, NoopShell, Shell, ShellEvent, TextInputState};
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
// `TurApp` instance (engine on a worker thread; `HostBackend` owns the renderer
// on the host thread and drives it directly).
pub use crate::core::runtime::{HostBackend, TurAppBuilder, TurRuntime, TurRuntimeBuilder};
// Re-export the plugin-layer main-thread hop surface so backends in other
// crates (`tur-clipboard-native`, future OS-API backends) can name the type
// without reaching into `core::plugin`. OS-API backends receive an
// `HostExecutor` clone at construction (via the closure form of
// `TurRuntimeBuilder::capability`) and hop OS-API calls onto the engine's
// main thread (required by some platforms — e.g. macOS
// `arboard`/`NSPasteboard`); plugin code reaches the same channel via
// `PluginContext::to_host_executor`. The engine creates the channel internally at
// `build()`, so no embedder wiring is required.
pub use crate::core::plugin::{HostExecutor, HostRunFuture};
pub use crate::core::scheduler::SpawnError;
// Re-export the module-source registry so embedders can write
// `tur_engine::ModuleSourceRegistry` — the handle-based module-loading path
// (register an `Arc<str>` once, load it into any instance by opaque id,
// never crossing an embedder boundary as a string). Owned by embedder-side
// runtime state (e.g. `AndroidRuntime`), not by `TurRuntime` itself.
pub use crate::core::app::ModuleSourceRegistry;
// Re-export the worker-pool declaration type so embedders can write
// `tur_engine::WorkerPoolHandle` (registered via
// `TurRuntimeBuilder::worker_pool`, assigned via `TurAppBuilder::worker_pool`).
pub use crate::core::scheduler::WorkerPoolHandle;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use error::TurError;

use core::app::FrameOutcome;

/// A running tur engine instance.
///
/// Wraps a [`HostBackend`] that owns a worker thread (running a
/// [`WorkerBackend`](core::runtime::WorkerBackend)) **and** the host-side
/// renderer + shell (both passed to `TurRuntime::app_builder()...build(...)`).
/// Main drives the renderer directly from [`HostBackend`]; shell requests
/// (`HostMsg::Shell`) are applied there too.
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
///   cooperatively on the JS event loop (the host thread never blocks).
/// - On native (Android JNI, integration tests), `futures::executor::block_on`
///   parks the calling thread until the future resolves.
pub struct TurApp {
    backend: HostBackend,
    /// Per-instance frame cadence. Cloned from the runtime at
    /// construction; embedders with per-instance cadence (Android's
    /// per-`FrameLoop` sources) replace it via [`Self::set_vsync_source`]
    /// before `run_loop` starts.
    vsync: RefCell<Rc<dyn core::scheduler::VsyncSource>>,
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

impl TurApp {
    /// Construct a `TurApp` backed by the given [`HostBackend`] + vsync
    /// source. `pub(crate)`: instances are only constructed by
    /// [`TurRuntime::app_builder`](crate::core::runtime::TurRuntime::app_builder)
    /// → [`TurAppBuilder::build`](crate::core::runtime::TurAppBuilder::build);
    /// — embedders never call it directly.
    pub(crate) fn new(backend: HostBackend, vsync: Rc<dyn core::scheduler::VsyncSource>) -> Self {
        Self {
            backend,
            vsync: RefCell::new(vsync),
            after_frame: RefCell::new(None),
            loop_started: Cell::new(false),
            destroyed: Cell::new(false),
        }
    }

    /// Replace the per-instance vsync source. Used by embedders that need
    /// per-instance frame cadence (e.g. Android, where each instance has
    /// its own JNI `FrameLoop`). Call after
    /// `runtime.app_builder().build(...)` and **before** `run_loop()` —
    /// the loop subscribes to the installed source once at startup.
    pub fn set_vsync_source(&self, source: Rc<dyn core::scheduler::VsyncSource>) {
        *self.vsync.borrow_mut() = source;
    }

    /// Direct accessor on the underlying [`HostBackend`]. Embedders use it
    /// to install the cursor backend after construction.
    pub fn backend(&self) -> &HostBackend {
        &self.backend
    }

    /// Handle-based module load: resolve `handle` in `registry` and load
    /// the shared source via [`HostBackend::load_module`] (parse + evaluate
    /// as an ES module and invoke its `start()` export — the module
    /// lifecycle contract: `start` returns an optional cleanup function;
    /// the engine runs it before the next load and at destroy).
    ///
    /// The natural pair for [`ModuleSourceRegistry`] — embedders that
    /// register sources Rust-side (APK assets, bundle files) load them by
    /// opaque id, so the source never crosses an embedder boundary as a
    /// string. String-based embedders (the wasm host, tests) call
    /// [`HostBackend::load_module`] directly.
    ///
    /// An unknown / released handle is an error (never UB — registry handles
    /// are monotonic ids, so a stale value can only miss).
    pub async fn load_module_source(
        &self,
        registry: &ModuleSourceRegistry,
        handle: u64,
    ) -> Result<(), TurError> {
        let source = registry
            .get(handle)
            .ok_or_else(|| TurError::Other(format!("unknown module source handle: {handle}")))?;
        self.backend
            .load_module(source)
            .await
            .map_err(TurError::from)
    }

    /// Read rendered pixels back from the owned renderer (screenshot
    /// tests). Returns `None` if the renderer doesn't support readback.
    pub fn render_to_pixels(&self) -> Option<Vec<u8>> {
        self.backend.render_to_pixels()
    }

    /// Cross-thread-safe event bus handle. `emit_to_js` ships via the
    /// worker's channel; JS→host messages fire handlers registered via
    /// `on_bus_event` (shipped back as `HostMsg::EventBusToEmbedder`).
    pub fn event_bus_handle(&self) -> core::event_bus::EventBusHandle {
        self.backend.event_bus_handle()
    }

    /// Push a platform (input) event from the embedder — resize, pointer,
    /// wheel, key, IME, or paste. Accepts anything that converts into a
    /// [`PlatformEvent`](core::platform::PlatformEvent): pass a
    /// [`ShellEvent`](core::shell::ShellEvent) directly for raw input, or
    /// a `Custom` payload wrapper for domain events. Re-arms an idle
    /// autonomous loop.
    pub fn push_platform_event(&self, event: impl Into<core::platform::PlatformEvent>) {
        self.backend
            .send_worker_msg(core::app::WorkerMsg::PlatformEvent(event.into()));
        self.request_frame();
    }

    /// Resize the surface. The embedder calls this at resize-event-receipt
    /// time (DOM `ResizeObserver` / winit / JNI): it resizes the host-side
    /// renderer directly (no flush + worker→host round-trip — lower
    /// latency) AND forwards the shell `Resize` event to the worker so
    /// `ResizeSubsystem` updates `Screen` / `viewportSize$` for layout.
    /// Event-driven, not per-frame, so no dedup is needed.
    pub fn resize(&self, logical_width: u32, logical_height: u32, dpr: f64) {
        self.backend.resize(logical_width, logical_height, dpr);
        self.backend
            .send_worker_msg(core::app::WorkerMsg::PlatformEvent(
                core::platform::PlatformEvent::Shell(core::shell::ShellEvent::Resize {
                    logical_width,
                    logical_height,
                    dpr,
                }),
            ));
        self.request_frame();
    }

    /// Push an engine-internal event onto the app-event bus (programmatic
    /// scrolls, clipboard writes). Re-arms an idle autonomous loop.
    pub fn push_app_event(&self, event: core::app::AppEvent) {
        self.backend
            .send_worker_msg(core::app::WorkerMsg::AppEvent(event));
        self.request_frame();
    }

    /// The autonomous frame loop — driven by the embedder's platform loop
    /// (Choreographer-polled on Android, `spawn_local`'d on wasm). The
    /// engine owns all frame logic; the platform only supplies the wake-up
    /// cadence via [`VsyncSource::subscribe`](core::scheduler::VsyncSource::subscribe)
    /// + [`VsyncSource::request_frame`](core::scheduler::VsyncSource::request_frame).
    ///
    /// Each iteration races the platform vsync stream against the worker's
    /// `HostMsg` stream:
    /// - **vsync** — kick the worker for the NEXT frame (it flushes+records
    ///   N+1 while main encodes N), then paint the latest buffered batch
    ///   (vsync-aligned, latest-wins).
    /// - **worker msg** — dispatch via [`HostBackend::apply_msg`](core::runtime::HostBackend::apply_msg),
    ///   the single shared handler. `RenderCommands` is buffered into
    ///   `pending` for vsync-aligned pipelining; `FrameOutcome` fires the
    ///   `after_frame` hook and re-arms vsync (or flushes `pending` on
    ///   quiescence). Side-effects (shell commands, image uploads) are
    ///   applied inside `apply_msg`.
    ///
    /// The bootstrap is automatic: `app_builder().build(...)` pushes an
    /// initial resize event to the worker, the worker pumps + ships
    /// `FrameOutcome` back, and the loop requests the next vsync based on
    /// the outcome. No initial `request_frame()` is needed.
    ///
    /// Concurrency: single-loop serialized. The embedder must spawn this
    /// future exactly once per `TurApp`. Multiple concurrent calls panic.
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn run_loop(self: Rc<Self>) {
        assert!(
            !self.loop_started.replace(true),
            "run_loop called twice on the same TurApp"
        );

        let mut vsync_rx = self.vsync.borrow().subscribe();
        // `host_rx` is in a RefCell; borrow for the lifetime of this loop.
        // Safe: run_loop is called exactly once per app (asserted above).
        #[allow(clippy::await_holding_refcell_ref)]
        let mut host_rx = self.backend.host_rx.borrow_mut();

        use crate::core::runtime::MsgOutcome;
        use futures::future::{Either, select};
        use futures::stream::StreamExt;

        // Pipelining buffer: the latest un-rendered batch from the worker.
        let mut pending: Option<core::render::RenderCommandBatch> = None;

        loop {
            // Race vsync + host_msg streams — first to fire wins.
            let vsync_fut = vsync_rx.next();
            let main_fut = host_rx.next();
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
                                self.vsync.borrow().request_frame();
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

    /// Re-arm an idle autonomous loop: ask the vsync source for one
    /// wake-up on the next frame. Idempotent at the source (armed flag).
    fn request_frame(&self) {
        self.vsync.borrow().request_frame();
    }

    /// Test-only: run `cb` against the worker's live `NodeTreeData` AND
    /// `FocusManager`. This is the general escape hatch the former
    /// per-field queries (`focused_cursor_rect`, `focused_is_editable`,
    /// `dev_tool_element_tree`, `dev_tool_get_element`) were folded into —
    /// callers reconstruct those views from the raw pair (the tree's
    /// `dev_tool_node` / `root_element_id`, focus's `focused()`, the
    /// `focused_is_editable` helper in [`core::focus::helper`]).
    ///
    /// Returns `None` if the worker is gone (`R: Send + 'static`,
    /// `cb: Send + 'static`). Production code should never call this.
    pub async fn with_tree<R: Send + 'static>(
        &self,
        cb: impl FnOnce(&core::elements::NodeTreeData, &core::focus::FocusManager) -> R + Send + 'static,
    ) -> Option<R> {
        use core::app::comm::{Reply, TreeRunner, WorkerMsg};

        let (tx, rx) = Reply::<Option<R>>::pair();
        let runner: TreeRunner = Box::new(move |tree, focus| {
            tx.send(Some(cb(tree, focus)));
        });
        let _ = self
            .backend
            .worker_tx()
            .unbounded_send(WorkerMsg::WithTree { runner });
        rx.rx.await.unwrap_or(None)
    }
}
