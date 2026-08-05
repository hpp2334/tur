//! Platform scheduling primitives.
//!
//! Two traits split by thread context:
//! - [`MainScheduler`] — main-thread surface (spawn_worker, vsync, request_vsync).
//! - [`WorkerScheduler`] — worker-thread surface (spawn_local, sleep).
//!
//! The same driver object implements both; the runtime holds two `Rc<dyn>`
//! trait objects pointing at it. Thread-locals inside the impl dispatch
//! `spawn_local` to the right per-thread `LocalPool`.
//!
//! ## Dependency direction
//!
//! Engine → scheduler, one-way. Drivers have zero engine knowledge — they
//! expose primitives (spawn, vsync events, sleep futures) and the engine
//! drives itself via [`crate::TurApp::start_loop`].

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use futures::Stream;
use futures::future::{AbortHandle, Abortable};

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

/// Handle to a `spawn_local`-ed task. Cheap to construct; dropping it
/// **detaches** the task (the future keeps running, you just can't join
/// or abort it anymore).
///
/// - [`abort`](Self::abort) cancels the task: its future is dropped at the
///   next poll point, freeing any resources it held (pending `Sleep`s,
///   promise slots, etc.).
/// - [`join`](Self::join) awaits the task's completion (whether it finished
///   naturally or was aborted).
///
/// Built generically by [`track_spawn`] around any platform spawn, so all
/// drivers (wasm `wasm_bindgen_futures`, native `LocalPool`, test `LocalSet`)
/// share one implementation. The handle is `!Send` (single-threaded
/// executors; the engine's `TurApp` is `Rc`-based anyway).
pub struct TaskHandle {
    abort_handle: AbortHandle,
    join_rx: RefCell<Option<futures::channel::oneshot::Receiver<()>>>,
}

impl TaskHandle {
    /// Cancel the task. Its future is dropped at the next poll point.
    /// Idempotent — calling after the task already completed is a no-op.
    pub fn abort(&self) {
        self.abort_handle.abort();
    }

    /// Await the task's completion. Consumes the handle. Resolves on both
    /// natural completion and abort. Returns `None` if the task was
    /// dropped without completing (e.g. the executor shut down).
    pub async fn join(self) -> Option<()> {
        // Take the receiver out of the RefCell BEFORE awaiting so the
        // RefCell borrow isn't held across the await point. The `let`
        // statement ends the temporary `RefMut` at its `;`.
        let rx = self.join_rx.borrow_mut().take();
        match rx {
            Some(rx) => rx.await.ok(),
            None => None,
        }
    }
}

/// Wrap a future so it is abortable + joinable, then hand the wrapped
/// future to a platform spawn function (`wasm_bindgen_futures::spawn_local`,
/// a `LocalPool`/`LocalSet` spawn, etc.). Returns a [`TaskHandle`] that
/// can abort or await the task.
///
/// The wrapper pairs `futures::future::Abortable` (cancel signal) with a
/// oneshot (completion signal) — both pure-Rust and executor-independent,
/// so no driver needs executor-level task-handle support.
pub fn track_spawn(
    fut: Pin<Box<dyn Future<Output = ()> + 'static>>,
    spawn: impl FnOnce(Pin<Box<dyn Future<Output = ()> + 'static>>),
) -> TaskHandle {
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let (tx, rx) = futures::channel::oneshot::channel();
    let tracked: Pin<Box<dyn Future<Output = ()> + 'static>> = Box::pin(async move {
        // `Abortable::await` resolves Ok(()) on natural completion or
        // Err(Aborted) on abort — either way the task has stopped, so
        // signal the joiner.
        let _ = Abortable::new(fut, abort_registration).await;
        let _ = tx.send(());
    });
    spawn(tracked);
    TaskHandle {
        abort_handle,
        join_rx: RefCell::new(Some(rx)),
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
    /// (`std::thread` on native, Web Worker via `wasm_thread` on wasm) and
    /// **returns the worker's main future** (the engine's `worker_loop`).
    /// The driver sets up thread-locals (e.g. the LocalPool) on the worker
    /// thread *before* invoking the factory, then constructs a
    /// `Rc<dyn WorkerScheduler>` for that thread and passes it to the
    /// factory, and finally drives the returned future the way that
    /// platform keeps a worker alive:
    ///
    /// - **Native** (std::thread): drive the future to completion on the
    ///   worker thread's LocalPool (an infinite loop → the thread blocks
    ///   forever, polling the loop + all `spawn_local`'d side tasks).
    /// - **Wasm** (Web Worker): root the future on the JS event loop via
    ///   `spawn_local` and return — the worker stays alive because the
    ///   loop's pending channel-waker keeps event-loop tasks scheduled.
    ///
    /// The returned future runs on the worker thread only (never crosses
    /// threads), so it is `+ 'static` but not `Send`; the factory closure
    /// itself must be `Send + 'static` (it crosses main → worker and may
    /// capture only `Send + Sync` config).
    #[allow(clippy::type_complexity)]
    fn spawn_worker(
        &self,
        factory: Box<
            dyn FnOnce(Rc<dyn WorkerScheduler>) -> Pin<Box<dyn Future<Output = ()> + 'static>>
                + Send
                + 'static,
        >,
    ) -> WorkerHandle;

    /// Spawn a future on the main thread's local executor. Returns a
    /// [`TaskHandle`] that can abort or await the task; drop it to detach.
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle;

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
///
/// There is deliberately **no `block_on`**: blocking the calling thread on
/// a future cannot be implemented honestly on wasm (you cannot block; the
/// idiomatic pattern is `spawn_local` + event-loop driving). The engine's
/// worker loop is driven by [`MainScheduler::spawn_worker`], which moves
/// the per-platform "how to keep the worker alive" decision into the
/// driver.
pub trait WorkerScheduler: 'static {
    /// Spawn a future on this worker thread's local executor. Returns a
    /// [`TaskHandle`] that can abort or await the task; drop it to detach.
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle;

    /// Create a Sleep future.
    fn sleep(&self, d: Duration) -> Sleep;
}
