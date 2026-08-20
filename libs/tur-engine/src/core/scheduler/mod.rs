//! Platform scheduling contract — worker vocabulary, zero thread concepts.
//!
//! Four single-role traits, one per concern. Platform layers implement
//! them; the engine consumes them. Nothing here knows about OS threads,
//! lanes, Web Workers, or `block_on` — "how a worker stays alive" is
//! entirely a platform-layer detail:
//!
//! - [`WorkerSpawner`] (runtime-level) — host app loops in named
//!   [`WorkerPoolHandle`] pools.
//! - [`VsyncSource`] (per-instance) — frame cadence: subscribe + arm.
//!   Swappable per app via [`crate::TurApp::set_vsync_source`].
//! - [`HostLoop`] (runtime-level) — spawn tasks on the host thread (the
//!   platform main thread; drives the engine's internal host-thread drain).
//! - [`WorkerExecutor`] (worker-side) — the surface an app loop runs on:
//!   `spawn_local` / `spawn_blocking` / `sleep`. Every method is live on
//!   every platform — no `unimplemented!` stubs anywhere.
//!
//! The engine never blocks a worker: the only primitives worker-side code
//! may use are spawn + await (`block_on` on a worker would stall every
//! co-tenant app sharing it).
//!
//! ## Dependency direction
//!
//! Engine → scheduler, one-way. Implementations have zero engine
//! knowledge — they expose primitives (spawn, vsync events, sleep
//! futures) and the engine drives itself via [`crate::TurApp::run_loop`].

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

/// A boxed, `!Send` future runnable on the thread it was created on
/// (worker lane or host thread).
pub type LocalFut = Pin<Box<dyn Future<Output = ()> + 'static>>;

/// Newtype around a boxed future. Implementations construct it from their
/// platform-specific timer primitive (setTimeout on wasm, tokio::time::sleep
/// on native, virtual clock on tests); consumers just `.await` it.
pub struct Sleep(pub Pin<Box<dyn Future<Output = ()> + 'static>>);

impl Future for Sleep {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        self.0.as_mut().poll(cx)
    }
}

/// Stream of vsync events. Each item is one vsync tick. The source pushes
/// events into the underlying channel when the platform fires rAF /
/// Choreographer; the engine reads them inside [`crate::TurApp::run_loop`].
///
/// Events only fire when armed via [`VsyncSource::request_frame`].
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

// ---------------------------------------------------------------------------
// Worker hosting — runtime-level
// ---------------------------------------------------------------------------

/// Host app loops in worker pools. Runtime-level: the engine calls
/// [`WorkerSpawner::spawn_worker`] exactly once per app, from
/// `TurRuntime::app_builder().worker_pool(pool)…build()`.
///
/// The spawner picks or creates a worker in `pool` (grow-to-cap-then-
/// least-loaded; see [`pool`]), delivers the [`WorkerEntry`] there, and
/// returns a [`WorkerTicket`] claiming one slot on that worker.
///
/// Platform implementations:
/// - **Native** ([`tur_native::worker_pool::NativeWorkerPools`]): capped
///   shared OS "lane" threads; each app's loop future is pinned to one
///   lane for its lifetime (`!Send` state: boa `Context`, `Rc`s) and lanes
///   run multiple app loops cooperatively.
/// - **Wasm** (`tur_wasm::scheduler::WasmWorkerSpawner`): capped shared Web
///   Workers; each hosts multiple app loops on one JS event loop
///   (multi-tenant workers, factory delivery via `postMessage`).
///
/// Apps in different pools never share workers. A cap ≥ the app count
/// degenerates to one-worker-per-app.
pub trait WorkerSpawner: 'static {
    /// Host one app loop in `pool`. The `entry` closure runs on the chosen
    /// worker (native lane thread / Web Worker — platform-defined), builds
    /// the `!Send` engine backend there, and returns the app's run-loop
    /// future; the platform then drives that future for the worker's
    /// lifetime. The closure itself is `Send + 'static` (it crosses
    /// host-thread → worker and may capture only `Send` config); the
    /// returned future runs on the worker only.
    ///
    /// ## Readiness contract
    ///
    /// Implementations that can block the calling thread (native) MUST
    /// return only after the entry's synchronous prologue — backend
    /// construction + plugin `register` — completed, so `spawn_worker`
    /// returning guarantees the worker's plugin-level side effects are
    /// observable. Implementations on non-blocking hosts (wasm: the entry
    /// is delivered as a message and the JS main thread cannot block)
    /// return immediately; embedders confirm readiness by awaiting the
    /// first RPC instead.
    fn spawn_worker(&self, pool: &WorkerPoolHandle, entry: WorkerEntry) -> WorkerTicket;
}

/// The engine's per-app worker entry: runs on the chosen worker, receives
/// the worker's [`WorkerContext`], builds the `!Send` backend (boa
/// `Context`, `Rc`s) there, and returns the app's run-loop future (the
/// engine's `worker_loop`).
pub type WorkerEntry = Box<dyn FnOnce(WorkerContext) -> LocalFut + Send + 'static>;

/// Claim on one app's slot in a worker. Returned by
/// [`WorkerSpawner::spawn_worker`]; held for the app's lifetime.
///
/// Two faces, both required by the engine:
/// - `join` — signals **that app's loop** completed (not the underlying
///   worker, which may host co-tenant apps that keep it alive).
/// - `wake` — cross-thread kick, called by the engine after every
///   host→worker channel send. No-op on native (the mpsc waker unparks
///   the OS thread directly); `worker.postMessage(0)` on wasm (the only
///   way to rouse an idle Web Worker's JS event loop without a sync
///   `Atomics.wait`, which would freeze it).
///
/// `wake` is `Rc<dyn Fn>` (`!Send`) because wasm implementations capture
/// a `web_sys::Worker` handle that lives only on the host thread.
pub struct WorkerTicket {
    join: Box<dyn FnOnce()>,
    wake: Rc<dyn Fn()>,
}

impl WorkerTicket {
    /// Construct with a no-op cross-thread wake (native — the OS thread
    /// parks on its own scheduler and wakes via the mpsc waker).
    pub fn new(join: Box<dyn FnOnce()>) -> Self {
        Self {
            join,
            wake: Rc::new(|| {}),
        }
    }

    /// Construct with a non-trivial cross-thread `wake` (wasm — installs a
    /// `worker.postMessage(0)` kick).
    pub fn with_wake(join: Box<dyn FnOnce()>, wake: Rc<dyn Fn()>) -> Self {
        Self { join, wake }
    }

    /// Wait until this app's loop completes (native blocks; wasm
    /// decrements the worker's live-app count and terminates it at zero).
    pub fn join(self) {
        (self.join)()
    }

    /// Clone of the cross-thread wake callback. The engine's `HostBackend`
    /// calls it after every host→worker send.
    pub fn wake(&self) -> Rc<dyn Fn()> {
        self.wake.clone()
    }
}

// ---------------------------------------------------------------------------
// Vsync — per-instance
// ---------------------------------------------------------------------------

/// Frame cadence for one app. Per-instance: each [`crate::TurApp`] holds
/// one and the embedder may replace it after build (Android installs one
/// bound to the instance's own Kotlin `FrameLoop` via
/// [`crate::TurApp::set_vsync_source`]) — swap before
/// [`crate::TurApp::run_loop`] starts, since the loop subscribes once at
/// startup.
pub trait VsyncSource: 'static {
    /// Subscribe to vsync events. Each item is one vsync tick. Call once
    /// at loop startup; events only fire when armed via
    /// [`Self::request_frame`].
    fn subscribe(&self) -> VsyncEvents;

    /// Arm the next vsync. Idempotent — multiple calls before the next
    /// vsync are coalesced into one rAF/Choreographer request.
    fn request_frame(&self);
}

// ---------------------------------------------------------------------------
// Host thread — runtime-level
// ---------------------------------------------------------------------------

/// Spawn tasks on the host thread's executor (the host thread is the
/// platform main thread). Runtime-level; the engine uses it exactly once
/// (rooting its internal host-thread drain at `build()`); embedders may
/// use it for their own host-thread tasks.
///
/// The engine core itself drives [`crate::TurApp::run_loop`] directly
/// (`wasm_bindgen_futures::spawn_local` on wasm, JNI `pump` polling on
/// Android, `block_on` in tests) — this trait exists for tasks the engine
/// must root, not for the frame loop.
pub trait HostLoop: 'static {
    fn spawn_local(&self, fut: LocalFut) -> TaskHandle;
}

// ---------------------------------------------------------------------------
// Worker-side surface
// ---------------------------------------------------------------------------

/// A `Send` closure with CPU-heavy or blocking work, run off the worker's
/// own loop by [`WorkerExecutor::spawn_blocking`].
pub type BlockingWork = Box<dyn FnOnce() + Send + 'static>;

/// Future returned by [`WorkerExecutor::spawn_blocking`]. Resolves when
/// the blocking work finished.
///
/// Panics if the work panicked or was dropped before completing (the
/// platform implementation contains the panic off-thread and cancels the
/// channel); on a lane executor the panic is contained to this awaiting
/// task, not the app loop.
pub struct BlockingSpawn(pub Pin<Box<dyn Future<Output = ()> + 'static>>);

impl Future for BlockingSpawn {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        self.0.as_mut().poll(cx)
    }
}

/// The worker-side executor surface an app loop (and everything it
/// spawns) runs on. Every method is live on every platform:
///
/// - `spawn_local` — cooperative task on the worker's own loop (native:
///   the lane's task table; wasm: `wasm_bindgen_futures::spawn_local`).
/// - `spawn_blocking` — run [`BlockingWork`] OFF the worker's loop.
///   Native: a dedicated OS thread (completion wakes the awaiting task
///   through its normal waker). Wasm has no honest blocking pool, so the
///   **default** implementation runs the work as its own cooperative task
///   on the worker's event loop — co-tenants keep getting poll slots, but
///   the work itself still occupies the thread while running (documented
///   approximation, not an illusion of offload).
/// - `sleep` — platform timer (`setTimeout` on wasm, tokio on native,
///   virtual clock in tests).
///
/// There is deliberately **no `block_on`**: blocking the worker would
/// stall every co-tenant app sharing it. Await spawned work instead.
pub trait WorkerExecutor: 'static {
    /// Spawn a cooperative task on this worker's loop. Returns a
    /// [`TaskHandle`] that can abort or await the task; drop to detach.
    fn spawn_local(&self, fut: LocalFut) -> TaskHandle;

    /// Run blocking work off this worker's loop. See the trait docs for
    /// platform semantics.
    fn spawn_blocking(&self, work: BlockingWork) -> BlockingSpawn {
        // Default (wasm-honest): the work becomes its own task on this
        // executor. Completion resolves a oneshot that wakes the awaiting
        // task through the normal waker path.
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        let _ = self.spawn_local(Box::pin(async move {
            work();
            let _ = tx.send(());
        }));
        BlockingSpawn(Box::pin(async move {
            let _ = rx.await;
        }))
    }

    /// Create a Sleep future.
    fn sleep(&self, d: Duration) -> Sleep;
}

/// Worker-side scheduling surface handed to each [`WorkerEntry`] and
/// threaded through the engine (`TurInstanceContext`,
/// `SubsystemFlushContext`, `AsyncWorkerContext`). Cheap to clone
/// (Rc-backed).
#[derive(Clone)]
pub struct WorkerContext {
    executor: Rc<dyn WorkerExecutor>,
}

impl WorkerContext {
    /// Wrap a worker executor. Called by the platform's
    /// [`WorkerSpawner::spawn_worker`] implementation when it hands the
    /// engine its per-worker scheduling surface.
    pub fn new(executor: Rc<dyn WorkerExecutor>) -> Self {
        Self { executor }
    }

    /// Spawn a cooperative task on this worker's loop.
    pub fn spawn_local(&self, fut: LocalFut) -> TaskHandle {
        self.executor.spawn_local(fut)
    }

    /// Run blocking work off this worker's loop. See
    /// [`WorkerExecutor::spawn_blocking`] for platform semantics.
    ///
    /// One method covers both shapes: `work` accepts a bare closure
    /// (`move || ...`) **or** an already-boxed callback (`Box<dyn FnOnce() ->
    /// T + Send>` — boxed closures implement `FnOnce`). The closure's return
    /// value resolves to the awaiting task through the returned
    /// [`BlockingResult<T>`] (use `T = ()` for unit work). Panics (contained
    /// to the awaiting task) if the work panicked or was dropped.
    pub fn spawn_blocking<T>(&self, work: impl FnOnce() -> T + Send + 'static) -> BlockingResult<T>
    where
        T: Send + 'static,
    {
        let (tx, rx) = futures::channel::oneshot::channel::<T>();
        self.executor.spawn_blocking(Box::new(move || {
            let _ = tx.send(work());
        }));
        BlockingResult {
            fut: Box::pin(async move {
                match rx.await {
                    Ok(v) => v,
                    Err(_) => panic!("spawn_blocking work panicked or was dropped"),
                }
            }),
        }
    }

    /// Create a Sleep future on this worker.
    pub fn sleep(&self, d: Duration) -> Sleep {
        self.executor.sleep(d)
    }
}

/// Future returned by [`WorkerContext::spawn_blocking`]. Resolves to the
/// closure's return value; panics (contained to the awaiting task) if the
/// work panicked.
pub struct BlockingResult<T> {
    fut: Pin<Box<dyn Future<Output = T>>>,
}

impl<T> Future for BlockingResult<T> {
    type Output = T;
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<T> {
        self.fut.as_mut().poll(cx)
    }
}

// ---------------------------------------------------------------------------
// Task handles (shared by every spawn surface)
// ---------------------------------------------------------------------------

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
/// around any platform spawn, so all executors share one implementation.
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
/// a lane task-table insert, etc.). Returns a [`TaskHandle<T>`] that
/// can abort or await the task.
///
/// The wrapper pairs `futures::future::Abortable` (cancel signal) with a
/// oneshot carrying `Result<T, SpawnError>` (completion signal) — both
/// pure-Rust and executor-independent, so no executor needs executor-level
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

// ---------------------------------------------------------------------------
// Host-thread task hop — raw mechanics (pub(crate))
// ---------------------------------------------------------------------------
//
// The plugin-layer abstraction over these is `HostExecutor`
// (`core/plugin.rs`), which wraps the sender half and exposes
// `run_on_host` / `run_on_host_async` / `spawn_on_host`. The engine creates
// the channel here in `TurRuntimeBuilder::build` and roots the drain on
// the host thread via the runtime's [`HostLoop`]. Keeping the raw channel
// in the scheduler module (not the plugin module) preserves the dependency
// direction: plugin → scheduler.

/// A boxed, `Send` future runnable on the host thread. Crosses the worker →
/// host boundary, so it must be `Send` (a stronger bound than a single-threaded
/// `spawn_local` requires).
pub(crate) type HostTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Receiver side of the host-thread channel. Spawned on the host thread once
/// (via [`HostLoop::spawn_local`]) at runtime `build()`; runs received tasks
/// inline, in arrival order. When the last sender (the
/// [`HostExecutor`](crate::core::plugin::HostExecutor)) is
/// dropped the channel closes and [`HostDrain::run`] completes.
pub(crate) struct HostDrain {
    pub(crate) rx: mpsc::UnboundedReceiver<HostTask>,
}

/// Create the `(sender, drain)` pair. The engine wraps the sender into an
/// [`HostExecutor`](crate::core::plugin::HostExecutor) (plugin
/// layer) and roots the drain on the host thread in `build()`.
pub(crate) fn host_channel() -> (mpsc::UnboundedSender<HostTask>, HostDrain) {
    let (tx, rx) = mpsc::unbounded();
    (tx, HostDrain { rx })
}

impl HostDrain {
    /// Run the drain loop: receive tasks and `await` them, one at a time,
    /// until the last sender is dropped. Must be spawned on the **host
    /// thread** (via [`HostLoop::spawn_local`]) so the tasks' OS-API
    /// calls execute there.
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
    use futures::executor::block_on;

    /// `track_spawn` returns `Ok(t)` for a value-producing future joined
    /// naturally, and the generic `TaskHandle<T>` carries the output. The
    /// `HostExecutor` (host-thread hop) round-trip tests live in
    /// `core/plugin.rs` next to that type.
    #[test]
    fn task_handle_join_returns_value() {
        let handle: TaskHandle<u32> = track_spawn(Box::pin(async { 99 }), |f| {
            // Drive the future to completion synchronously.
            block_on(f);
        });
        assert_eq!(block_on(handle.join()), Ok(99));
    }

    /// The default `WorkerExecutor::spawn_blocking` runs work as its own
    /// cooperative task and resolves the returned future — pinned with a
    /// minimal inline executor.
    struct InlineExecutor;

    impl WorkerExecutor for InlineExecutor {
        fn spawn_local(&self, fut: LocalFut) -> TaskHandle {
            track_spawn(fut, |f| {
                block_on(f);
            })
        }
        fn sleep(&self, _d: Duration) -> Sleep {
            Sleep(Box::pin(std::future::pending::<()>()))
        }
        fn spawn_blocking(&self, work: BlockingWork) -> BlockingSpawn {
            // Exercise the exact default body (copied) so the test pins it.
            let (tx, rx) = futures::channel::oneshot::channel::<()>();
            let _ = self.spawn_local(Box::pin(async move {
                work();
                let _ = tx.send(());
            }));
            BlockingSpawn(Box::pin(async move {
                let _ = rx.await;
            }))
        }
    }

    #[test]
    fn default_spawn_blocking_resolves() {
        use std::sync::{Arc, Mutex};
        let ran = Arc::new(Mutex::new(false));
        let ran_for_work = ran.clone();
        let cx = WorkerContext::new(Rc::new(InlineExecutor));
        // Boxed callback (`Box<dyn FnOnce() + Send>`) — the dyn-safe shape a
        // bridge might hand over — flows through the same method.
        let work: BlockingWork = Box::new(move || {
            *ran_for_work.lock().unwrap() = true;
        });
        block_on(cx.spawn_blocking(work));
        assert!(*ran.lock().unwrap(), "blocking work ran via default impl");
    }

    #[test]
    fn spawn_blocking_carries_value() {
        let cx = WorkerContext::new(Rc::new(InlineExecutor));
        let result = block_on(cx.spawn_blocking(|| 40 + 2));
        assert_eq!(result, 42);
    }
}
