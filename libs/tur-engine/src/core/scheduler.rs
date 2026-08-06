//! Platform scheduling primitives.
//!
//! Two driver traits split by thread context, each wrapped in a concrete
//! view struct that the engine holds:
//! - [`MainSchedulerDriver`] / [`MainScheduler`] — main-thread surface
//!   (`spawn_worker`, `vsync_events`, `request_vsync`, `spawn_local`,
//!   `sleep`).
//! - [`WorkerSchedulerDriver`] / [`WorkerScheduler`] — worker-thread
//!   surface (`spawn_local`, `sleep`).
//!
//! The view structs (`MainScheduler`, `WorkerScheduler`) each hold an
//! `Rc<dyn …Driver>` and delegate. The underlying driver objects are
//! platform-specific: a main driver (`WasmSchedulerDriver` /
//! `AndroidSchedulerDriver` / `TestSchedulerDriver`) impls
//! `MainSchedulerDriver`, and a separate per-worker driver
//! (`WasmWorkerScheduler` etc., constructed inside `spawn_worker` on the
//! worker thread) impls `WorkerSchedulerDriver`. They are genuinely
//! different objects — the main driver is `!Send` (stays on main); the
//! worker driver is built fresh on each worker thread.
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
///
/// `notify` is a cross-thread wake callback the embedder calls after every
/// main→worker channel send. On native it is a no-op — the mpsc waker
/// unparks the worker's OS thread directly. On wasm it is
/// `worker.postMessage(0)` — the only way to kick an idle Web Worker's JS
/// event loop without a sync `Atomics.wait` (which would freeze the loop).
/// The callback is `Rc<dyn Fn>` (`!Send`) because it captures a
/// `web_sys::Worker` handle that lives only on the main thread.
pub struct WorkerHandle {
    join: Box<dyn FnOnce()>,
    notify: Rc<dyn Fn()>,
}

impl WorkerHandle {
    pub fn new(join: Box<dyn FnOnce()>) -> Self {
        Self {
            join,
            notify: Rc::new(|| {}),
        }
    }

    /// Construct with a non-trivial cross-thread `notify` wake (used by the
    /// wasm driver to install a `worker.postMessage(0)` kick).
    pub fn with_notify(join: Box<dyn FnOnce()>, notify: Rc<dyn Fn()>) -> Self {
        Self { join, notify }
    }

    pub fn join(self) {
        (self.join)()
    }

    /// Clone of the cross-thread wake callback. `MainBackend` calls this
    /// after every main→worker send.
    pub fn notify(&self) -> Rc<dyn Fn()> {
        self.notify.clone()
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

/// Worker-thread driver trait. Impl'd by the per-worker driver structs
/// (`WasmWorkerScheduler`, `AndroidWorkerScheduler`,
/// `TestWorkerScheduler`) constructed inside
/// [`MainSchedulerDriver::spawn_worker`] on the worker thread. These are
/// the only scheduling primitives valid on a worker thread.
///
/// `spawn_local`/`sleep` have identical signatures to the ones on
/// [`MainSchedulerDriver`] — both sides support them, but the driver
/// objects differ, so the two traits are independent and self-contained
/// (a shared super-trait would break `dyn` dispatch: super-trait methods
/// are not in a `dyn MainSchedulerDriver` vtable without nightly
/// upcasting, so the main view couldn't call them).
pub trait WorkerSchedulerDriver: 'static {
    /// Spawn a future on this worker thread's local executor. Returns a
    /// [`TaskHandle`] that can abort or await the task; drop it to detach.
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle;

    /// Create a Sleep future.
    fn sleep(&self, d: Duration) -> Sleep;
}

/// Worker-thread scheduling view. Concrete struct wrapping a
/// [`WorkerSchedulerDriver`]. Cheap to clone (inner `Rc`); held by
/// `TurJsContext`, `SubsystemFlushContext`, `TurAppInternal`, and passed
/// to bridges.
///
/// There is deliberately **no `block_on`**: blocking the calling thread on
/// a future cannot be implemented honestly on wasm (you cannot block; the
/// idiomatic pattern is `spawn_local` + event-loop driving). The engine's
/// worker loop is driven by [`MainSchedulerDriver::spawn_worker`], which
/// moves the per-platform "how to keep the worker alive" decision into the
/// main driver.
#[derive(Clone)]
pub struct WorkerScheduler {
    driver: Rc<dyn WorkerSchedulerDriver>,
}

impl WorkerScheduler {
    /// Wrap a worker driver. Called by the main driver's `spawn_worker`
    /// impl (in each platform crate) to hand the engine its per-thread
    /// scheduling handle.
    pub fn new(driver: Rc<dyn WorkerSchedulerDriver>) -> Self {
        Self { driver }
    }

    /// Spawn a future on this worker thread's local executor. Returns a
    /// [`TaskHandle`] that can abort or await the task; drop it to detach.
    pub fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        self.driver.spawn_local(fut)
    }

    /// Create a Sleep future.
    pub fn sleep(&self, d: Duration) -> Sleep {
        self.driver.sleep(d)
    }
}

/// The engine's `worker_loop` factory: runs on the worker thread,
/// constructs the `WorkerBackend` (`!Send` types built there), returns the
/// worker's main future. `Send + 'static` so it can be boxed on main and
/// moved across to the worker thread (native `std::thread`) or reconstituted
/// from a raw pointer (wasm shared linear memory).
pub type WorkerFactory = Box<
    dyn FnOnce(WorkerScheduler) -> Pin<Box<dyn Future<Output = ()> + 'static>> + Send + 'static,
>;

/// Main-thread driver trait. Impl'd by the platform's main driver
/// (`WasmSchedulerDriver`, `AndroidSchedulerDriver`,
/// `TestSchedulerDriver`). The runtime holds one `Rc<dyn
/// MainSchedulerDriver>` (wrapped in [`MainScheduler`]).
///
/// Methods here are valid only when called from the main thread. The main
/// driver is `!Send` (it may hold `Rc`/`RefCell`/JNI handles), so it stays
/// on main for the app's lifetime; per-instance replacement (Android's
/// per-`FrameLoop` driver) goes through [`crate::TurApp::set_main_scheduler`].
pub trait MainSchedulerDriver: 'static {
    /// Spawn a worker. The factory runs on a new worker thread
    /// (`std::thread` on native, Web Worker via the in-tree spawner on
    /// wasm) and **returns the worker's main future** (the engine's
    /// `worker_loop`). The driver sets up thread-locals (e.g. the
    /// LocalPool) on the worker thread *before* invoking the factory,
    /// then constructs a [`WorkerScheduler`] for that thread (wrapping a
    /// fresh per-worker driver) and passes it to the factory, and finally
    /// drives the returned future the way that platform keeps a worker
    /// alive:
    ///
    /// - **Native** (std::thread): drive the future to completion on the
    ///   worker thread's LocalPool (an infinite loop → the thread blocks
    ///   forever, polling the loop + all `spawn_local`'d side tasks).
    /// - **Wasm** (Web Worker): the worker drives its `loop_fut`
    ///   cooperatively on the JS event loop (a mini single-task executor —
    ///   no `block_on`, no `Atomics.wait` freeze). Cross-thread wake is
    ///   `worker.postMessage(0)` (the returned [`WorkerHandle`]'s `notify`
    ///   callback); same-thread wake is a `setTimeout(0)` repoll.
    ///
    /// The returned future runs on the worker thread only (never crosses
    /// threads), so it is `+ 'static` but not `Send`; the factory closure
    /// itself must be `Send + 'static` (it crosses main → worker and may
    /// capture only `Send + Sync` config).
    fn spawn_worker(&self, factory: WorkerFactory) -> WorkerHandle;

    /// Subscribe to vsync events. Each item is one vsync tick.
    /// Call once at engine startup (inside `TurApp::start_loop`).
    /// Events only fire when armed via [`Self::request_vsync`].
    fn vsync_events(&self) -> VsyncEvents;

    /// Arm the next vsync. Idempotent — multiple calls before the next
    /// vsync are coalesced into one rAF/Choreographer request (kills the
    /// rAF churn perf bug as a side effect).
    fn request_vsync(&self);

    /// Spawn a future on the main thread's local executor. Returns a
    /// [`TaskHandle`] that can abort or await the task; drop it to detach.
    /// The engine core does not currently call this on the main thread
    /// (the autonomous `start_loop` future is driven directly by the
    /// embedder — `wasm_bindgen_futures::spawn_local` on wasm, JNI
    /// `nativePump` on Android, `block_on` in tests), but it is available
    /// for embedders/main-thread code that needs it.
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle;

    /// Create a Sleep future. Implementation is platform-specific. Like
    /// [`Self::spawn_local`], not used by engine core on the main thread
    /// but available for main-thread callers that need it.
    fn sleep(&self, d: Duration) -> Sleep;
}

/// Main-thread scheduling view. Concrete struct wrapping a
/// [`MainSchedulerDriver`]. Cheap to clone (inner `Rc`); held by
/// `TurRuntime`, `MainBackend`, and `TurApp` (the latter in a `RefCell`
/// so Android can swap the per-instance driver).
#[derive(Clone)]
pub struct MainScheduler {
    driver: Rc<dyn MainSchedulerDriver>,
}

impl MainScheduler {
    /// Wrap a main driver. Called by the runtime builder internally and by
    /// embedders that replace the per-instance driver
    /// ([`crate::TurApp::set_main_scheduler`]).
    pub fn new(driver: Rc<dyn MainSchedulerDriver>) -> Self {
        Self { driver }
    }

    /// Spawn a worker. See [`MainSchedulerDriver::spawn_worker`].
    pub fn spawn_worker(&self, factory: WorkerFactory) -> WorkerHandle {
        self.driver.spawn_worker(factory)
    }

    /// Subscribe to vsync events. Each item is one vsync tick.
    pub fn vsync_events(&self) -> VsyncEvents {
        self.driver.vsync_events()
    }

    /// Arm the next vsync. Idempotent — coalesces into one rAF/Choreographer.
    pub fn request_vsync(&self) {
        self.driver.request_vsync()
    }

    /// Spawn a future on the main thread's local executor.
    pub fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        self.driver.spawn_local(fut)
    }

    /// Create a Sleep future on the main thread.
    pub fn sleep(&self, d: Duration) -> Sleep {
        self.driver.sleep(d)
    }
}
