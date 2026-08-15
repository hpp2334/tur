//! Platform scheduling primitives.
//!
//! Two driver traits split by thread context, each wrapped in a concrete
//! view struct that the engine holds:
//! - [`MainSchedulerDriver`] / [`MainScheduler`] — main-thread surface
//!   (`spawn_worker_in`, `vsync_events`, `request_vsync`, `spawn_local`,
//!   `sleep`).
//! - [`WorkerSchedulerDriver`] / [`WorkerScheduler`] — worker-thread
//!   surface (`spawn_local`, `sleep`).
//!
//! The view structs (`MainScheduler`, `WorkerScheduler`) each hold an
//! `Rc<dyn …Driver>` and delegate. The underlying driver objects are
//! platform-specific: a main driver (`WasmSchedulerDriver` /
//! `AndroidSchedulerDriver` / `TestSchedulerDriver`) impls
//! `MainSchedulerDriver`, and a separate per-worker driver
//! (`WasmWorkerScheduler` etc., constructed inside `spawn_worker_in` on the
//! worker thread) impls `WorkerSchedulerDriver`. They are genuinely
//! different objects — the main driver is `!Send` (stays on main); the
//! worker driver is built fresh on each worker thread.
//!
//! ## Worker pools
//!
//! Workers are spawned *into* a named pool ([`WorkerPoolHandle`]) via
//! [`MainSchedulerDriver::spawn_worker_in`]; the engine calls it once per
//! app from `TurRuntime::app_builder().worker_pool(pool)…build()` with the
//! embedder-assigned pool. Pooling itself is platform-implemented — the
//! engine only defines the contract:
//!
//! - **Native** (`tur-native::NativeWorkerPools`): at most
//!   `max_threads` OS "lane" threads per pool; each app's `worker_loop`
//!   future is pinned to one lane for its lifetime (`!Send` state: boa
//!   `Context`, `Rc`s) and lanes run multiple app loops cooperatively.
//! - **Wasm** (`tur-wasm`): at most `max_threads` Web Workers per pool,
//!   each hosting multiple app loops on one JS event loop
//!   (multi-tenant workers, factory delivery via `postMessage`).
//!
//! A cap ≥ the app count degenerates to one-worker-per-app (the historical
//! default). Apps in different pools never share threads — that isolation
//! is the point: heavy daemon JS in a `daemon` pool cannot stall UI
//! rendering in a `ui` pool.
//!
//! ## Dependency direction
//!
//! Engine → scheduler, one-way. Drivers have zero engine knowledge — they
//! expose primitives (spawn, vsync events, sleep futures) and the engine
//! drives itself via [`crate::TurApp::run_loop`].

pub mod pool;

pub use pool::WorkerPoolHandle;

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use futures::Stream;
use futures::StreamExt;
use futures::channel::mpsc;
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
/// Choreographer; the engine reads them inside [`crate::TurApp::run_loop`].
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

/// Returned by [`MainScheduler::spawn_worker_in`]. Held for the worker's
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

/// Why a [`TaskHandle<T>`]'s `join` did not yield a value.
///
/// `join` returns `Result<T, SpawnError>`: `Ok(t)` on natural completion,
/// `Err(Aborted)` when the task was canceled via [`TaskHandle::abort`], and
/// `Err(Dropped)` when the task's future was dropped before it could complete
/// (executor shutdown, or `join` called twice on the same handle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    /// The task was canceled via [`TaskHandle::abort`].
    Aborted,
    /// The task was dropped before completing — the executor shut down, or
    /// the handle's oneshot was already consumed (double-join).
    Dropped,
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpawnError::Aborted => write!(f, "task was aborted"),
            SpawnError::Dropped => write!(f, "task was dropped before completing"),
        }
    }
}

impl std::error::Error for SpawnError {}

/// Handle to a `spawn_local`-ed task. Cheap to construct; dropping it
/// **detaches** the task (the future keeps running, you just can't join
/// or abort it anymore).
///
/// Generic over the task future's output `T` (defaults to `()`); [`join`]
/// returns `Result<T, SpawnError>`. Built generically by [`track_spawn`]
/// around any platform spawn, so all drivers (wasm `wasm_bindgen_futures`,
/// native `LocalPool`, test `LocalSet`) share one implementation.
///
/// - [`abort`](Self::abort) cancels the task: its future is dropped at the
///   next poll point, freeing any resources it held (pending `Sleep`s,
///   promise slots, etc.).
/// - [`join`](Self::join) awaits the task's completion: `Ok(t)` on natural
///   finish, `Err(SpawnError::Aborted)` if aborted, `Err(SpawnError::Dropped)`
///   if the future was dropped before completing.
///
/// The handle is `!Send` (single-threaded executors; the engine's `TurApp`
/// is `Rc`-based anyway).
///
/// [`join`]: Self::join
pub struct TaskHandle<T = ()> {
    abort_handle: AbortHandle,
    join_rx: RefCell<Option<futures::channel::oneshot::Receiver<Result<T, SpawnError>>>>,
}

impl<T> TaskHandle<T> {
    /// Cancel the task. Its future is dropped at the next poll point.
    /// Idempotent — calling after the task already completed is a no-op.
    pub fn abort(&self) {
        self.abort_handle.abort();
    }

    /// Await the task's completion. Consumes the handle.
    ///
    /// - `Ok(t)` — the task finished and produced `t`.
    /// - `Err(SpawnError::Aborted)` — the task was canceled via [`abort`](Self::abort).
    /// - `Err(SpawnError::Dropped)` — the future was dropped before it could
    ///   complete (executor shutdown), or `join` was already called.
    pub async fn join(self) -> Result<T, SpawnError> {
        // Take the receiver out of the RefCell BEFORE awaiting so the
        // RefCell borrow isn't held across the await point. The `let`
        // statement ends the temporary `RefMut` at its `;`.
        let rx = self.join_rx.borrow_mut().take();
        match rx {
            // `rx.await` is `Result<Result<T, SpawnError>, Canceled>`:
            //   Ok(inner) → inner carries Ok(t) | Err(Aborted) (sent by the
            //               tracked wrapper based on Abortable's result);
            //   Err(_)    → the wrapper future was dropped before sending
            //               (executor shutdown) → Dropped.
            Some(rx) => rx.await.unwrap_or(Err(SpawnError::Dropped)),
            // Handle already consumed by a prior join.
            None => Err(SpawnError::Dropped),
        }
    }
}

/// Wrap a future so it is abortable + joinable, then hand the wrapped
/// future to a platform spawn function (`wasm_bindgen_futures::spawn_local`,
/// a `LocalPool`/`LocalSet` spawn, etc.). Returns a [`TaskHandle<T>`] that
/// can abort or await the task.
///
/// The wrapper pairs `futures::future::Abortable` (cancel signal) with a
/// oneshot carrying `Result<T, SpawnError>` (completion signal) — both
/// pure-Rust and executor-independent, so no driver needs executor-level
/// task-handle support. The abort/drop distinction is made inside the
/// wrapper: `Abortable`'s `Err(Aborted)` becomes `SpawnError::Aborted`;
/// the wrapper future being dropped (executor shutdown) surfaces as
/// `SpawnError::Dropped` at the joiner (a canceled oneshot).
pub fn track_spawn<T: Send + 'static>(
    fut: Pin<Box<dyn Future<Output = T> + 'static>>,
    spawn: impl FnOnce(Pin<Box<dyn Future<Output = ()> + 'static>>),
) -> TaskHandle<T> {
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let (tx, rx) = futures::channel::oneshot::channel::<Result<T, SpawnError>>();
    let tracked: Pin<Box<dyn Future<Output = ()> + 'static>> = Box::pin(async move {
        // `Abortable::await` resolves Ok(t) on natural completion or
        // Err(Aborted) on abort. Forward either to the joiner; the
        // Aborted variant is what lets join() distinguish abort from drop.
        let result = Abortable::new(fut, abort_registration)
            .await
            .map_err(|_| SpawnError::Aborted);
        let _ = tx.send(result);
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
/// [`MainSchedulerDriver::spawn_worker_in`] on the worker thread. These are
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
/// `TurInstanceContext`, `SubsystemFlushContext`, `TurAppInternal`, and passed
/// to bridges.
///
/// There is deliberately **no `block_on`**: blocking the calling thread on
/// a future cannot be implemented honestly on wasm (you cannot block; the
/// idiomatic pattern is `spawn_local` + event-loop driving). The engine's
/// worker loop is driven by [`MainSchedulerDriver::spawn_worker_in`], which
/// moves the per-platform "how to keep the worker alive" decision into the
/// main driver.
#[derive(Clone)]
pub struct WorkerScheduler {
    driver: Rc<dyn WorkerSchedulerDriver>,
}

impl WorkerScheduler {
    /// Wrap a worker driver. Called by the main driver's `spawn_worker_in`
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
    /// Spawn a worker into `pool`. The factory runs on the chosen worker
    /// thread (`std::thread` lane on native, Web Worker via the in-tree
    /// spawner on wasm) and **returns the worker's main future** (the
    /// engine's `worker_loop`). The driver sets up thread-locals (e.g. the
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
    /// ## Pool semantics
    ///
    /// The worker the factory lands on is pinned to `pool`:
    /// - If the pool has fewer live workers than `pool.max_threads()`, the
    ///   driver may create a fresh one (grow-to-cap).
    /// - Otherwise the driver must host the new app's loop on an existing
    ///   worker of that pool (least-loaded), scheduled cooperatively with
    ///   the apps already there.
    ///
    /// Apps in *different* pools never share workers. Since app state
    /// (`boa::Context`, `Rc`s) is `!Send`, each app's loop runs on exactly
    /// one worker thread for its entire lifetime.
    ///
    /// The returned future runs on the worker thread only (never crosses
    /// threads), so it is `+ 'static` but not `Send`; the factory closure
    /// itself must be `Send + 'static` (it crosses main → worker and may
    /// capture only `Send + Sync` config).
    ///
    /// The default impl panics: drivers that never opted into worker pools
    /// (e.g. Android's per-instance vsync-only replacement drivers) don't
    /// spawn workers themselves. The platform embedder's runtime-level
    /// driver is expected to implement it (native: compose
    /// `tur_native::NativeWorkerPools`; wasm: built into
    /// `WasmSchedulerDriver`).
    fn spawn_worker_in(&self, pool: &WorkerPoolHandle, factory: WorkerFactory) -> WorkerHandle {
        let _ = (pool, factory);
        panic!(
            "this MainSchedulerDriver does not implement worker pools; \
             implement spawn_worker_in or compose tur_native::NativeWorkerPools"
        );
    }

    /// Subscribe to vsync events. Each item is one vsync tick.
    /// Call once at engine startup (inside `TurApp::run_loop`).
    /// Events only fire when armed via [`Self::request_vsync`].
    fn vsync_events(&self) -> VsyncEvents;

    /// Arm the next vsync. Idempotent — multiple calls before the next
    /// vsync are coalesced into one rAF/Choreographer request (kills the
    /// rAF churn perf bug as a side effect).
    fn request_vsync(&self);

    /// Return a `Send + Sync` callback that arms a main-thread vsync, for
    /// Spawn a future on the main thread's local executor. Returns a
    /// [`TaskHandle`] that can abort or await the task; drop it to detach.
    /// The engine core does not currently call this on the main thread
    /// (the autonomous `run_loop` future is driven directly by the
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

    /// Spawn a worker into `pool`. See
    /// [`MainSchedulerDriver::spawn_worker_in`].
    pub fn spawn_worker_in(&self, pool: &WorkerPoolHandle, factory: WorkerFactory) -> WorkerHandle {
        self.driver.spawn_worker_in(pool, factory)
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

// ---------------------------------------------------------------------------
// Main-thread task hop — raw mechanics (pub(crate))
// ---------------------------------------------------------------------------
//
// The plugin-layer abstraction over these is `AsyncPluginContext`
// (`core/plugin.rs`), which wraps the sender half and exposes
// `run_on_main` / `run_on_main_async` / `spawn_on_main`. The engine creates
// the channel here in `TurRuntimeBuilder::build` and spawns the drain on the
// main thread. Keeping the raw channel in the scheduler module (not the
// plugin module) preserves the dependency direction: plugin → scheduler.

/// A boxed, `Send` future runnable on the main thread. Crosses the worker →
/// main boundary, so it must be `Send` (a stronger bound than a single-threaded
/// `spawn_local` requires).
pub(crate) type MainTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Receiver side of the main-thread channel. Spawned on the main thread once
/// (via
/// [`MainScheduler::spawn_local`](crate::core::scheduler::MainScheduler::spawn_local))
/// at runtime `build()`; runs received tasks inline, in arrival order. When
/// the last sender (the [`AsyncPluginContext`](crate::core::plugin::AsyncPluginContext))
/// is dropped the channel closes and [`MainDrain::run`] completes.
pub(crate) struct MainDrain {
    pub(crate) rx: mpsc::UnboundedReceiver<MainTask>,
}

/// Create the `(sender, drain)` pair. The engine wraps the sender into an
/// [`AsyncPluginContext`](crate::core::plugin::AsyncPluginContext) (plugin
/// layer) and spawns the drain on the main thread in `build()`.
pub(crate) fn main_channel() -> (mpsc::UnboundedSender<MainTask>, MainDrain) {
    let (tx, rx) = mpsc::unbounded();
    (tx, MainDrain { rx })
}

impl MainDrain {
    /// Run the drain loop: receive tasks and `await` them, one at a time,
    /// until the last sender is dropped. Must be spawned on the **main
    /// thread** (via
    /// [`MainScheduler::spawn_local`](crate::core::scheduler::MainScheduler::spawn_local))
    /// so the tasks' OS-API calls execute there.
    ///
    /// Tasks run **inline + serialized**: a blocking OS call briefly delays
    /// the next queued task. This is intentional — OS APIs are often
    /// non-reentrant, and serialization avoids reentrancy bugs.
    pub(crate) async fn run(mut self) {
        while let Some(task) = self.rx.next().await {
            task.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `track_spawn` returns `Ok(t)` for a value-producing future joined
    /// naturally, and the generic `TaskHandle<T>` carries the output. The
    /// `AsyncPluginContext` (main-thread hop) round-trip tests live in
    /// `core/plugin.rs` next to that type.
    #[test]
    fn task_handle_join_returns_value() {
        use futures::executor::block_on;

        let handle: TaskHandle<u32> = track_spawn(Box::pin(async { 99 }), |f| {
            // Drive the future to completion synchronously.
            block_on(f);
        });
        assert_eq!(block_on(handle.join()), Ok(99));
    }
}
