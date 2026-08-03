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
// `runtime.create_app(viewport, dpr)` spawns an isolated `TurApp` instance
// (engine on a worker thread; embedder installs a `render_sink` on main to
// receive command batches + drive its own renderer).
pub use crate::core::runtime::{MainBackend, RenderSink, TurRuntime, TurRuntimeBuilder};

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use error::TurError;

use core::app::FrameOutcome;

/// Snapshot of focused-element state — single struct for the two-value
/// `focused_is_editable` + `focused_cursor_rect` pair. Used by
/// [`TurApp::focused_state`] (RPC) and [`TurApp::cached_focus`] (cached).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FocusedState {
    pub is_editable: bool,
    /// Logical-space `(x, y, w, h)` of the focused element's caret, or
    /// `None` if no editable is focused.
    pub cursor_rect: Option<(f64, f64, f64, f64)>,
}

/// Type-erased future driven by the embedder's async runtime. The wake
/// trampoline (set via [`LoopDriver::set_wake`] inside [`TurApp::start`])
/// hands one of these to the embedder-supplied `spawn` closure — which
/// either `wasm_bindgen_futures::spawn_local`s it (wasm main thread,
/// cooperatively scheduled on the JS event loop) or
/// `futures::executor::block_on`s it (native, calling thread parks until
/// the future completes).
pub type WakeFuture = Pin<Box<dyn Future<Output = ()>>>;

/// Type-erased spawn closure passed by the embedder to
/// [`TurApp::start`]. Receives a future; the embedder decides how to drive
/// it (`spawn_local` / `block_on` / custom executor).
pub type SpawnWake = Rc<dyn Fn(WakeFuture)>;

/// A running tur engine instance.
///
/// Wraps a [`MainBackend`] that owns a worker thread (running a
/// [`WorkerBackend`](core::runtime::WorkerBackend)). The embedder drives
/// rendering on the main thread via [`Self::set_render_sink`]; everything
/// else (boa `Context`, element tree, reactive store, layout, subsystems)
/// lives on the worker.
///
/// Construct via [`TurRuntime::create_app`].
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
    /// Autonomous-loop driver. `None` until [`Self::start`] is called
    /// (production); tests leave it unset and pump via [`Self::pump`].
    driver: RefCell<Option<Rc<dyn LoopDriver>>>,
    /// Embedder-supplied spawn closure — drives the async wake trampoline
    /// on the platform's runtime. Set once in [`Self::start`]; held so
    /// [`Self::spawn_wake`] (the concurrency-guarded entry point used by
    /// the wake trampoline) can re-invoke it for the next frame.
    spawn: RefCell<Option<SpawnWake>>,
    /// Embedder-installed callback fired after each autonomous frame —
    /// typically used for DOM side-effects (file-pick resolution, textarea
    /// focus / caret positioning). `None` in tests.
    after_frame: RefCell<Option<AfterFrameHook>>,
    /// Concurrency guard: true while a wake is in-flight. Prevents
    /// re-entry (e.g. rAF firing while the previous frame's future is
    /// still pending) from spawning overlapping pumps.
    pump_in_progress: Cell<bool>,
    /// Set when rAF fires while `pump_in_progress` is true. On wake exit,
    /// if true, another wake is spawned.
    wake_pending: Cell<bool>,
    /// Set by [`Self::destroy`]. Subsequent wake attempts short-circuit.
    destroyed: Cell<bool>,
}

/// Per-frame hook fired at the end of [`TurApp::wake`] (after `pump`,
/// before rescheduling). See [`TurApp::set_after_frame_hook`].
pub type AfterFrameHook = Rc<dyn Fn(FrameOutcome)>;

impl TurApp {
    /// Construct a `TurApp` backed by the given [`MainBackend`]. The runtime
    /// calls this from [`TurRuntime::create_app`]; embedders normally don't
    /// call it directly.
    pub fn new(backend: MainBackend) -> Self {
        Self {
            backend,
            driver: RefCell::new(None),
            spawn: RefCell::new(None),
            after_frame: RefCell::new(None),
            pump_in_progress: Cell::new(false),
            wake_pending: Cell::new(false),
            destroyed: Cell::new(false),
        }
    }

    /// Direct accessor on the underlying [`MainBackend`]. Embedders use it
    /// to install the render sink + cursor backend after construction.
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

    /// Advance exactly one frame: send `Wake` to the worker, await the
    /// next `MainMsg::FrameOutcome`. Any `RenderCommands` / `CursorChanged`
    /// / `FocusedStateChanged` arriving in the meantime are dispatched to
    /// the render sink / cursor backend / focus cache respectively.
    pub async fn pump(&self) -> Result<core::app::FrameOutcome, TurError> {
        self.backend.pump().await
    }

    /// Legacy alias for [`Self::pump`].
    pub async fn run_frame(&self) -> Result<core::app::FrameOutcome, TurError> {
        self.pump().await
    }

    /// Install the main-side render sink. The worker ships
    /// `Vec<RenderCommand>` + `Arc<ImageResourceMap>` + viewport tuple
    /// each frame; the sink applies them to its renderer.
    pub fn set_render_sink<
        F: FnMut(
                &[core::render::RenderCommand],
                &core::image_resource::ImageResourceMap,
                (u32, u32, f64),
            ) + 'static,
    >(
        &self,
        f: F,
    ) {
        self.backend.set_render_sink(f);
    }

    /// Cross-thread-safe event bus handle. `emit_to_js` ships via the
    /// worker's channel; `drain_js_to_host` returns empty on main (the
    /// worker emits `MainMsg::EventBusToHost` separately when needed).
    pub fn event_bus_handle(&self) -> core::event_bus::EventBusHandle {
        self.backend.event_bus_handle()
    }

    /// Combined focused-element state — RPC variant (awaits the worker's
    /// reply). For non-blocking reads, prefer [`Self::cached_focus`]
    /// (updated by the worker's deduped `MainMsg::FocusedStateChanged`).
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

    /// Push an engine-internal event onto the app-event bus (programmatic
    /// scrolls, clipboard writes). Re-arms an idle autonomous loop.
    pub fn push_app_event(&self, event: core::app::AppEvent) {
        self.backend
            .send_worker_msg(core::app::WorkerMsg::AppEvent(event));
        self.request_wakeup();
    }

    /// Request a paint on the next frame. Sets the `need_paint` flag
    /// directly on the worker. Re-arms an idle autonomous loop.
    pub fn request_paint(&self) {
        self.backend
            .send_worker_msg(core::app::WorkerMsg::RequestPaint);
        self.request_wakeup();
    }

    /// Begin autonomous operation: install the driver + the embedder's
    /// `spawn` closure, register the wake trampoline, and kick off frame 1.
    ///
    /// The wake trampoline the engine installs is a sync `Rc<dyn Fn()>`
    /// (matching the [`LoopDriver::set_wake`] contract). When the driver
    /// fires it (rAF / JNI / etc.), the trampoline wraps an async wake in
    /// a `Box::pin` and hands the future to `spawn`, which the embedder
    /// uses to drive the future on its platform's runtime.
    ///
    /// Concurrency: the trampoline guards against re-entry via
    /// `pump_in_progress` — if a wake fires while one is already
    /// in-flight, `wake_pending` is set and a fresh wake is spawned when
    /// the in-flight one resolves.
    pub fn start<S>(self: &Rc<Self>, driver: Rc<dyn LoopDriver>, spawn: S)
    where
        S: Fn(WakeFuture) + 'static,
    {
        let spawn: SpawnWake = Rc::new(spawn);
        *self.spawn.borrow_mut() = Some(spawn.clone());
        // Install the wake trampoline on the driver. When the driver fires
        // (rAF / JNI / etc.), it calls this trampoline, which hands a
        // fresh async wake to the embedder-supplied spawn closure.
        let weak = Rc::downgrade(self);
        driver.set_wake(Rc::new(move || {
            if let Some(app) = weak.upgrade() {
                app.spawn_wake();
            }
        }));
        *self.driver.borrow_mut() = Some(driver);

        // Kick off frame 1. spawn_wake sets pump_in_progress itself.
        self.spawn_wake();
    }

    /// Install a callback fired after each autonomous frame (in [`Self::wake`],
    /// after `pump`, before rescheduling).
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

    /// Re-arm an idle autonomous loop: ask the driver for one wake-up on
    /// the next frame. No-op when no driver is installed (tests) or when a
    /// frame is already pending.
    fn request_wakeup(&self) {
        if let Some(driver) = self.driver.borrow().as_ref() {
            driver.request_next(core::app::NextFrame::Vsync);
        }
    }

    /// Concurrency-guarded wake entry point. Spawns a fresh wake unless
    /// one is already in-flight (in which case `wake_pending` is set and
    /// the in-flight wake will pick it up on exit). The wake trampoline
    /// set in [`Self::start`] calls this.
    fn spawn_wake(self: &Rc<Self>) {
        if self.destroyed.get() {
            return;
        }
        if self.pump_in_progress.replace(true) {
            // Already in flight — defer.
            self.wake_pending.set(true);
            return;
        }
        let Some(spawn) = self.spawn.borrow().clone() else {
            // No spawn closure — must be a test path. Clear the guard.
            self.pump_in_progress.set(false);
            return;
        };
        let weak = Rc::downgrade(self);
        spawn(Box::pin(async move {
            if let Some(app) = weak.upgrade() {
                app.wake().await;
            }
        }));
    }

    /// One autonomous-frame tick: pump, the `after_frame` hook, then
    /// reschedule via the driver. Clears `pump_in_progress` and re-spawns
    /// if a wake was deferred during this tick.
    async fn wake(self: &Rc<Self>) {
        if self.destroyed.get() {
            self.pump_in_progress.set(false);
            return;
        }
        let outcome = match self.pump().await {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("frame loop pump error: {e}");
                self.pump_in_progress.set(false);
                return;
            }
        };
        if let Some(hook) = self.after_frame.borrow().as_ref().cloned() {
            hook(outcome);
        }
        if !self.destroyed.get() {
            if let Some(driver) = self.driver.borrow().as_ref().cloned() {
                driver.request_next(outcome.schedule);
            }
        }
        // Re-arm the deferred wake before clearing the in-flight guard
        // (the spawned wake will re-check `pump_in_progress`).
        let pending = self.wake_pending.replace(false);
        self.pump_in_progress.set(false);
        if pending {
            self.spawn_wake();
        }
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
                .map(|e| cb(e));
            tx.send(result);
        });
        // Best-effort send — returns Err if the worker is gone. In that case
        // we'll just await on `rx` which yields None.
        let _ = self
            .backend
            .worker_tx()
            .try_send(WorkerMsg::WithElement { id, runner });
        rx.rx.recv().await.unwrap_or(None)
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

    /// Latest cursor received from the worker (non-blocking). Updated
    /// during [`Self::pump`] when `MainMsg::CursorChanged` arrives.
    pub fn cached_cursor(&self) -> core::platform::Cursor {
        self.backend.cached_cursor()
    }

    /// Latest focus state received from the worker (non-blocking). Updated
    /// during [`Self::pump`] when `MainMsg::FocusedStateChanged` arrives.
    /// Useful for embedder callbacks (e.g. the wasm after-frame hook) that
    /// need focus info without an RPC round-trip.
    pub fn cached_focus(&self) -> FocusedState {
        self.backend.cached_focus()
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

/// Autonomous-loop driver — the platform scheduling primitive the engine
/// uses to wake itself for the next frame. Implementations live in the
/// embedder: a wasm driver backed by `requestAnimationFrame` / `setTimeout`
/// for the wake trampoline (tur-wasm), or any other platform's wake mechanism.
/// Tests do not install one (they pump [`TurApp::pump`] manually via
/// `block_on`).
pub trait LoopDriver {
    /// Install the engine's wake trampoline. The driver must call it exactly
    /// once whenever a wake-up requested via [`Self::request_next`] becomes
    /// due. Set once at [`TurApp::start`].
    ///
    /// The trampoline is sync (`Rc<dyn Fn()>`) but internally schedules an
    /// async wake via the spawn closure passed to `TurApp::start`. The
    /// driver sees only the sync surface.
    fn set_wake(&self, wake: Rc<dyn Fn()>);

    /// Request the next wake-up, replacing any pending request.
    /// - [`NextFrame::Vsync`](core::app::NextFrame) → wake on the next display
    ///   frame (~16 ms).
    /// - [`NextFrame::After(d)`](core::app::NextFrame) → wake after `d`.
    /// - [`NextFrame::Idle`](core::app::NextFrame) → cancel any pending
    ///   wake-up; the loop stops until [`TurApp::push_platform_event`] (via
    ///   `request_next(Vsync)`) re-arms it.
    fn request_next(&self, next: core::app::NextFrame);
}
