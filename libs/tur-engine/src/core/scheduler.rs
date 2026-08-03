//! Platform scheduling primitives.
//!
//! Two traits split by thread context:
//! - [`MainScheduler`] — main-thread surface (spawn_worker, vsync, request_vsync).
//! - [`WorkerScheduler`] — worker-thread surface (spawn_local, block_on, sleep).
//!
//! The same driver object implements both; the runtime holds two `Rc<dyn>`
//! trait objects pointing at it. Thread-locals inside the impl dispatch
//! `spawn_local` / `block_on` to the right per-thread `LocalPool`.
//!
//! ## Dependency direction
//!
//! Engine → scheduler, one-way. Drivers have zero engine knowledge — they
//! expose primitives (spawn, vsync events, sleep futures) and the engine
//! drives itself via [`crate::TurApp::start_loop`].

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use futures::Stream;

/// Newtype around a boxed future. Drivers construct it from their
/// platform-specific timer primitive (setTimeout on wasm, tokio::time::sleep
/// on native, virtual clock on tests); consumers just `.await` it.
pub struct Sleep(pub Pin<Box<dyn Future<Output = ()> + 'static>>);

impl Future for Sleep {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        self.0.as_mut().poll(cx)
    }
}

/// Stream of vsync events. Each item is one vsync tick. The driver pushes
/// events into the underlying channel when the platform fires rAF /
/// Choreographer; the engine reads them inside [`crate::TurApp::start_loop`].
///
/// Events only fire when armed via [`MainScheduler::request_vsync`].
pub struct VsyncEvents(pub futures::channel::mpsc::UnboundedReceiver<()>);

impl Stream for VsyncEvents {
    type Item = ();
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<()>> {
        Pin::new(&mut self.0).poll_next(cx)
    }
}

/// Returned by [`MainScheduler::spawn_worker`]. Held for the worker's
/// lifetime; `join()` blocks the caller until the worker exits.
pub struct WorkerHandle {
    join: Box<dyn FnOnce()>,
}

impl WorkerHandle {
    pub fn new(join: Box<dyn FnOnce()>) -> Self {
        Self { join }
    }
    pub fn join(self) {
        (self.join)()
    }
}

/// Main-thread scheduling surface. Methods here are valid only when called
/// from the main thread.
///
/// Implemented by `WasmSchedulerDriver` (tur-wasm), `AndroidSchedulerDriver`
/// (tur-android), and `TestSchedulerDriver` (tur-integration-tests). The
/// runtime holds an `Rc<dyn MainScheduler>`; `TurApp` clones it for its
/// autonomous loop.
pub trait MainScheduler: 'static {
    /// Spawn a worker. The factory runs on a new worker thread
    /// (`std::thread` on native, Web Worker via `wasm_thread` on wasm).
    /// The driver sets up thread-locals (e.g. the LocalPool) on the worker
    /// thread *before* invoking the factory, then constructs a
    /// `Rc<dyn WorkerScheduler>` for that thread and passes it to the
    /// factory. The factory uses it to call `block_on(worker_loop)` and
    /// to share with bridges via `PluginContext`.
    ///
    /// The factory must be `Send + 'static`. Capturing `Rc`/`!Send` state
    /// from main is not safe; capture only `Send + Sync` config (Arcs,
    /// config structs, tokio Handles). The `WorkerScheduler` argument is
    /// the canonical way for the factory to access worker-thread
    /// scheduling primitives.
    fn spawn_worker(
        &self,
        factory: Box<dyn FnOnce(Rc<dyn WorkerScheduler>) + Send + 'static>,
    ) -> WorkerHandle;

    /// Spawn a future on the main thread's local executor.
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>);

    /// Subscribe to vsync events. Each item is one vsync tick.
    /// Call once at engine startup (inside `TurApp::start_loop`).
    /// Events only fire when armed via [`Self::request_vsync`].
    fn vsync_events(&self) -> VsyncEvents;

    /// Arm the next vsync. Idempotent — multiple calls before the next
    /// vsync are coalesced into one rAF/Choreographer request (kills the
    /// rAF churn perf bug as a side effect).
    fn request_vsync(&self);

    /// Create a Sleep future. Implementation is platform-specific.
    fn sleep(&self, d: Duration) -> Sleep;
}

/// Worker-thread scheduling surface. Held by `WorkerBackend`; bridges grab
/// it from `PluginContext` / `SubsystemFlushContext`. Methods dispatch to
/// the *current thread's* executor (thread-local LocalPool), so the same
/// `Rc<dyn WorkerScheduler>` works on any worker thread.
pub trait WorkerScheduler: 'static {
    /// Spawn a future on this worker thread's local executor.
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>);

    /// Block the calling (worker) thread on a `()`-returning future. Drives
    /// both the future AND any `spawn_local`'d side-futures (LocalPool
    /// semantics). Used by `MainBackend` to drive `worker_loop` from the
    /// worker thread entry point.
    fn block_on(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>);

    /// Create a Sleep future.
    fn sleep(&self, d: Duration) -> Sleep;
}
