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
// `runtime.app_builder().renderer(r, viewport, dpr).build()` spawns an
// isolated `(Rc<TurApp>, TurAppLooper)` pair (engine on a worker thread;
// `HostBackend` owns the renderer on the host thread and drives it directly —
// the app handle carries the mid-loop `&self` surface, the looper carries the
// autonomous frame loop).
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
/// Wraps a [`HostBackend`] (shared with the instance's
/// [`TurAppLooper`]) that owns a worker thread (running a
/// [`WorkerBackend`](core::runtime::WorkerBackend)) **and** the host-side
/// renderer + shell (both passed to `TurRuntime::app_builder()...build(...)`).
/// Main drives the renderer directly from [`HostBackend`]; shell requests
/// (`HostMsg::Shell`) are applied there too.
/// Everything else (boa `Context`, element tree, reactive store, layout,
/// subsystems) lives on the worker.
///
/// Construct via [`TurRuntime::app_builder`] then
/// [`TurAppBuilder::build`](core::runtime::TurAppBuilder::build), which
/// returns `(Rc<TurApp>, TurAppLooper)` — the handle for the mid-loop
/// `&self` surface (input, RPC, destroy), the looper for the autonomous
/// frame loop. This split is the single-loop contract made structural:
/// the loop consumes its looper by value, so it can be spawned once and
/// only once.
///
/// ## Async API
///
/// All public methods are `async fn`. The embedder supplies the runtime:
/// - On wasm, `wasm_bindgen_futures::spawn_local` runs futures
///   cooperatively on the JS event loop (the host thread never blocks).
/// - On native (Android JNI, integration tests), `futures::executor::block_on`
///   parks the calling thread until the future resolves.
pub struct TurApp {
    /// Shared with [`TurAppLooper`] — the app sends (input, RPC) while
    /// the loop applies worker messages + renders. Every `HostBackend`
    /// method is `&self`, so `Rc`-sharing is borrow-free.
    backend: Rc<HostBackend>,
    /// Per-instance frame cadence, cloned from the runtime at construction.
    /// Shared with [`TurAppLooper`] (which subscribes once at `run` start
    /// and re-arms it on `FrameOutcome`s) because the app re-arms it from
    /// the input paths ([`Self::push_platform_event`], [`Self::resize`],
    /// [`Self::push_app_event`]) while the loop is running.
    vsync: Rc<RefCell<Rc<dyn core::scheduler::VsyncSource>>>,
    /// Set by [`Self::destroy`]. Shared with [`TurAppLooper`], whose vsync
    /// wake-ups poll it to stop after destroy.
    destroyed: Rc<Cell<bool>>,
}

/// Per-frame hook fired at the end of each iteration of
/// [`TurAppLooper::run`] (after the frame's render/schedule side-effects
/// are applied). See [`TurAppLooper::set_after_frame_hook`].
pub type AfterFrameHook = Rc<dyn Fn(FrameOutcome)>;

impl TurApp {
    /// Construct a `TurApp` sharing `backend` / `vsync` / `destroyed` with
    /// its looper. `pub(crate)`: instances are only constructed by
    /// [`TurRuntime::app_builder`](crate::core::runtime::TurRuntime::app_builder)
    /// → [`TurAppBuilder::build`](crate::core::runtime::TurAppBuilder::build);
    /// — embedders never call it directly.
    pub(crate) fn new(
        backend: Rc<HostBackend>,
        vsync: Rc<RefCell<Rc<dyn core::scheduler::VsyncSource>>>,
        destroyed: Rc<Cell<bool>>,
    ) -> Self {
        Self {
            backend,
            vsync,
            destroyed,
        }
    }

    /// Direct accessor on the underlying [`HostBackend`]. Shared with the
    /// instance's [`TurAppLooper`] — safe to call while the loop runs.
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

/// The autonomous frame-loop driver for one [`TurApp`] — the half of a
/// built instance that owns the worker→host message stream and races it
/// against the platform vsync source.
///
/// Produced together with the app handle by
/// [`TurAppBuilder::build`](core::runtime::TurAppBuilder::build) /
/// [`TurAppBuilder::build_headless`](core::runtime::TurAppBuilder::build_headless)
/// as `(Rc<TurApp>, TurAppLooper)`. The split makes the single-loop
/// contract structural:
///
/// - [`Self::run`] consumes the looper **by value**, so the returned
///   future owns its state — it is `'static` (spawnable via
///   `wasm_bindgen_futures::spawn_local`, type-erasable to
///   `Pin<Box<dyn Future>>` for Android's poll-per-`pump` driving) and a
///   second `run` is a compile error, not a runtime assert.
/// - Pre-run configuration ([`Self::set_vsync_source`],
///   [`Self::set_after_frame_hook`]) is exclusive (`&mut self`) and
///   structurally impossible after `run` takes the looper — matching the
///   build-time installment philosophy of
///   [`TurAppBuilder::shell`](core::runtime::TurAppBuilder::shell).
///
/// The app handle keeps the mid-loop `&self` surface (input, RPC,
/// `destroy`); the two share the `HostBackend`, the vsync source (the app
/// re-arms it from input paths while the loop runs) and the destroyed
/// flag.
pub struct TurAppLooper {
    /// Shared with the app handle — the app sends (input, RPC) while the
    /// loop applies worker messages + renders. Every `HostBackend` method
    /// is `&self`, so `Rc`-sharing is borrow-free.
    backend: Rc<HostBackend>,
    /// The worker→host message stream, drained **only** by this looper —
    /// owned outright, no `RefCell`, no borrow held across `.await`.
    host_rx: core::app::HostRx,
    /// Per-instance frame cadence, shared with the app handle (see
    /// [`TurApp`]'s field docs).
    vsync: Rc<RefCell<Rc<dyn core::scheduler::VsyncSource>>>,
    /// Set by `TurApp::destroy`; polled at each vsync wake-up.
    destroyed: Rc<Cell<bool>>,
    /// Embedder-installed callback fired after each autonomous frame —
    /// typically used for DOM side-effects (file-pick resolution,
    /// textarea focus / caret positioning). `None` in tests.
    after_frame: Option<AfterFrameHook>,
}

impl TurAppLooper {
    /// Construct the looper sharing `backend` / `vsync` / `destroyed` with
    /// its app handle. `pub(crate)`: built only by
    /// [`TurAppBuilder::build`](core::runtime::TurAppBuilder::build).
    pub(crate) fn new(
        backend: Rc<HostBackend>,
        host_rx: core::app::HostRx,
        vsync: Rc<RefCell<Rc<dyn core::scheduler::VsyncSource>>>,
        destroyed: Rc<Cell<bool>>,
    ) -> Self {
        Self {
            backend,
            host_rx,
            vsync,
            destroyed,
            after_frame: None,
        }
    }

    /// Replace the per-instance vsync source. Used by embedders that need
    /// per-instance frame cadence (e.g. Android, where each instance has
    /// its own JNI `FrameLoop`). Pre-run only: the loop subscribes to the
    /// installed source once at [`Self::run`] startup, and after that the
    /// looper is consumed.
    pub fn set_vsync_source(&mut self, source: Rc<dyn core::scheduler::VsyncSource>) {
        *self.vsync.borrow_mut() = source;
    }

    /// Install a callback fired at the end of each loop iteration, after
    /// the frame's render/schedule side-effects are applied. Pre-run only
    /// (see [`Self::set_vsync_source`]).
    pub fn set_after_frame_hook(&mut self, hook: Option<AfterFrameHook>) {
        self.after_frame = hook;
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
    ///   N+1 while main encodes N below), then paint the latest buffered batch
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
    /// Concurrency: single-loop serialized, enforced structurally — `run`
    /// consumes the looper by value, so the future exists at most once
    /// per instance.
    pub async fn run(self) {
        let Self {
            backend,
            mut host_rx,
            vsync,
            destroyed,
            after_frame,
        } = self;
        let mut vsync_rx = vsync.borrow().subscribe();

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
                    if destroyed.get() {
                        break;
                    }
                    // 1) Kick the worker for the NEXT frame first — it
                    //    flushes+records N+1 while main encodes N below.
                    backend.send_worker_msg(core::app::WorkerMsg::Wake);
                    // 2) Render the latest buffered batch (vsync-aligned,
                    //    latest-wins). Skip empty batches — an empty command
                    //    list paints a blank frame (clears the surface), which
                    //    is never desirable.
                    if let Some(batch) = pending.take().filter(|b| !b.is_empty()) {
                        backend.render_batch(&batch);
                    }
                }
                Either::Left((None, _)) => break,
                Either::Right((Some(msg), _)) => {
                    let stop = match backend.apply_msg(msg) {
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
                                vsync.borrow().request_frame();
                                false
                            } else if let Some(batch) = pending.take().filter(|b| !b.is_empty()) {
                                // Quiescence: no vsync is armed (nothing
                                // time-driven pending), so the pipeline
                                // would stall with an un-rendered batch
                                // (e.g. the initial frame, or a one-shot
                                // paint request). Flush it now (empty
                                // batches skipped — they'd paint blank) —
                                // the next frame only starts on a new input.
                                backend.render_batch(&batch);
                                false
                            } else {
                                // Idle + empty pending: no-op. The loop
                                // blocks on the next event.
                                false
                            };
                            if let Some(hook) = after_frame.as_ref() {
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
}
