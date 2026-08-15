//! Native worker-pool executor: capped shared "lane" threads.
//!
//! Implements the pool side of
//! [`MainSchedulerDriver::spawn_worker_in`](tur_engine::core::scheduler::MainSchedulerDriver::spawn_worker_in)
//! for native platforms. Embedder drivers (Android's JNI driver, the
//! integration-test driver, third-party drivers via
//! [`PooledSchedulerDriver`]) compose a [`NativeWorkerPools`] and delegate
//! `spawn_worker_in` to [`NativeWorkerPools::spawn`].
//!
//! ## Model
//!
//! Each pool owns at most `max_threads` **lane** OS threads ("tur-lane").
//! App assignment is grow-to-cap-then-least-loaded: the first
//! `max_threads` apps each get a fresh lane; later apps share the
//! least-loaded existing lane. Because engine app state (`boa::Context`,
//! `Rc`s) is `!Send`, each app's `worker_loop` future is pinned to exactly
//! one lane for its entire lifetime — "sharing" means multiple app loops
//! cooperatively scheduled on one thread, never migrated.
//!
//! ## Lane executor
//!
//! A lane is a hand-rolled single-threaded executor (the engine is
//! tokio-free by convention):
//!
//! - Task table: lane-thread-local `RefCell<HashMap<u64, TaskEntry>>`
//!   keyed by `u64`; entries are **taken out** of the table for polling so
//!   a poll that calls `spawn_local` can't hit a double borrow.
//! - Ready queue: `Arc<Mutex<VecDeque<u64>>>` + `Condvar`. Wakers push
//!   plain `u64` keys — never `Rc`s — so cross-thread wakes (futures-mpsc
//!   senders on other threads, timer threads) are sound. This mirrors the
//!   wasm driver's `NoopWaker` thread-id discipline.
//! - Idle: the lane parks on the condvar when the queue is empty; every
//!   wake-up path (task waker, spawn delivery, lane-handle drop) pushes a
//!   sentinel key + `notify_all`.
//! - `Sleep` is **not** reimplemented: it delegates to the platform's
//!   per-lane driver (`LaneDriverFactory`), whose impls are self-timing
//!   (tokio timers, virtual test clock) and wake the task via its `Waker`
//!   — which lands back in the ready queue.
//! - Panic containment: the app factory call and every task poll run under
//!   `catch_unwind`; a panicking app is removed (its `done` signal fires)
//!   while co-tenant apps on the same lane keep running.
//!
//! ## Lifecycle
//!
//! A lane exits when its spawn inbox is disconnected (all senders dropped
//! — the registry reaps dead lanes lazily at the next assignment) **and**
//! its task table is empty. `WorkerHandle::join` blocks on that app's own
//! loop completion (not the lane thread), so several apps on one lane can
//! be joined independently. [`NativeWorkerPools`] itself is main-thread
//! only (`RefCell` registry) — `spawn_worker_in` is only ever called from
//! `app_builder().build(...)` on the main thread.
//!
//! ## Fairness tradeoff
//!
//! Apps sharing a lane run cooperatively: a long synchronous JS flush in
//! one app stalls its lane-mates until it yields (awaits the next worker
//! message). That intra-lane coupling is the accepted boundary — the
//! guarantee pools provide is *between* pools (a busy `daemon` pool never
//! stalls a `ui` pool).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver as StdReceiver, Sender as StdSender, TryRecvError};
use std::sync::{Condvar, Mutex};
use std::task::{Context as TaskContext, Poll, Wake, Waker};
use std::time::Duration;

use tur_engine::core::scheduler::{
    MainSchedulerDriver, Sleep, TaskHandle, VsyncEvents, WorkerFactory, WorkerHandle,
    WorkerPoolHandle, WorkerScheduler, WorkerSchedulerDriver, track_spawn,
};

/// Builds the per-lane [`WorkerSchedulerDriver`] — the platform's `sleep`
/// backend + any thread-locals. The closure **runs on the fresh lane
/// thread** (never on main), so it may only capture `Send + Sync` state
/// (e.g. an `Arc<tokio Handle>` or the shared virtual-test-clock).
///
/// Example: the Android driver hands
/// `Arc::new(move || Rc::new(AndroidWorkerScheduler { runtime: handle.clone() }))`.
pub type LaneDriverFactory = Arc<dyn Fn() -> Rc<dyn WorkerSchedulerDriver> + Send + Sync>;

/// Sentinel ready-queue key: a "something happened" kick (spawn delivered,
/// lane handle dropped). Never a real task key — allocation skips it.
const SENTINEL: u64 = u64::MAX;

/// How many ready-task polls one scheduling pass may run before
/// re-checking the spawn inbox. Bounds starvation when a task synchronously
/// re-wakes itself; normal passes finish far earlier (empty queue).
const PASS_BUDGET: u32 = 128;

// ---------------------------------------------------------------------------
// Registry — main-thread object composed into a platform driver
// ---------------------------------------------------------------------------

/// Registry of worker pools → lane threads. Main-thread only (the registry
/// is a `RefCell`; `spawn` is called exclusively from
/// `MainSchedulerDriver::spawn_worker_in`, which the engine invokes on the
/// main thread during `app_builder().build(...)`).
///
/// Compose into a platform driver:
///
/// ```no_run
/// # use std::rc::Rc;
/// # use tur_native::worker_pool::{LaneDriverFactory, NativeWorkerPools};
/// # use tur_engine::core::scheduler::{
/// #     MainSchedulerDriver, Sleep, TaskHandle, VsyncEvents,
/// #     WorkerFactory, WorkerHandle, WorkerPoolHandle,
/// # };
/// # struct MyDriver { pools: Rc<NativeWorkerPools>, /* … */ }
/// impl MainSchedulerDriver for MyDriver {
///     fn spawn_worker_in(
///         &self,
///         pool: &WorkerPoolHandle,
///         factory: WorkerFactory,
///     ) -> WorkerHandle {
///         # let make_lane_driver: LaneDriverFactory = unreachable!();
///         self.pools.spawn(pool, factory, make_lane_driver)
///     }
///     # fn vsync_events(&self) -> VsyncEvents { unreachable!() }
///     # fn request_vsync(&self) {}
///     # fn spawn_local(&self, _: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle { unreachable!() }
///     # fn sleep(&self, _: Duration) -> Sleep { unreachable!() }
/// }
/// ```
///
/// Or wrap any native driver wholesale with [`PooledSchedulerDriver`].
pub struct NativeWorkerPools {
    pools: RefCell<Vec<PoolEntry>>,
}

struct PoolEntry {
    handle: WorkerPoolHandle,
    lanes: Vec<LaneHandle>,
}

impl Default for NativeWorkerPools {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeWorkerPools {
    pub fn new() -> Self {
        Self {
            pools: RefCell::new(Vec::new()),
        }
    }

    /// Spawn an app worker into `pool`: pick the least-loaded live lane, or
    /// grow a fresh one while the pool is under its `max_threads` cap. The
    /// factory runs on the lane thread and returns the app's
    /// `worker_loop` future; the returned [`WorkerHandle`] joins **that
    /// app's loop** (not the lane thread).
    ///
    /// A pool unseen by this registry is hosted on demand (fresh entry,
    /// zero lanes) — pool registration/identity was already validated by
    /// the engine (`TurAppBuilder` rejects unregistered handles), so this
    /// registry is purely a hosting detail.
    pub fn spawn(
        &self,
        pool: &WorkerPoolHandle,
        factory: WorkerFactory,
        lane_driver: LaneDriverFactory,
    ) -> WorkerHandle {
        let lane = {
            let mut pools = self.pools.borrow_mut();
            let entry = match pools.iter_mut().find(|e| e.handle.ptr_eq(pool)) {
                Some(entry) => entry,
                None => {
                    // First spawn into this pool — host it on demand.
                    pools.push(PoolEntry {
                        handle: pool.clone(),
                        lanes: Vec::new(),
                    });
                    pools.last_mut().expect("just pushed")
                }
            };
            // Lazily reap exited lanes (live == 0). Dropping the handle
            // drops the lane's last sender → its inbox disconnects → the
            // lane thread observes it and exits.
            entry.lanes.retain(|l| l.live.load(Ordering::Acquire) > 0);
            if entry.lanes.len() < pool.max_threads() {
                // Grow: first apps each get a fresh lane (max parallelism).
                let lane = LaneHandle::spawn(lane_driver);
                entry.lanes.push(lane.clone());
                lane
            } else {
                // Cap reached: share the least-loaded lane.
                entry
                    .lanes
                    .iter()
                    .min_by_key(|l| l.live.load(Ordering::Acquire))
                    .expect("cap >= 1 guarantees a live lane")
                    .clone()
            }
        };
        lane.spawn_app(factory)
    }
}

// ---------------------------------------------------------------------------
// Lane handle — main-side handle to one lane thread
// ---------------------------------------------------------------------------

enum LaneMsg {
    /// Deliver an app's worker factory; `done_tx` fires when that app's
    /// loop future completes (or the factory itself panics).
    SpawnApp {
        factory: WorkerFactory,
        done_tx: StdSender<()>,
    },
}

#[derive(Clone)]
struct LaneHandle {
    tx: StdSender<LaneMsg>,
    shared: LaneShared,
    live: Arc<AtomicUsize>,
}

impl Drop for LaneHandle {
    fn drop(&mut self) {
        // Kick the lane so a parked thread wakes up and observes the
        // disconnected inbox (all senders dropped) → exits. Harmless if
        // the lane is alive (a sentinel is just a wake-up).
        self.shared.push(SENTINEL);
    }
}

impl LaneHandle {
    fn spawn(lane_driver: LaneDriverFactory) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<LaneMsg>();
        let shared = LaneShared {
            ready: Arc::new(Mutex::new(VecDeque::new())),
            cv: Arc::new(Condvar::new()),
        };
        let live = Arc::new(AtomicUsize::new(0));
        let thread_shared = shared.clone();
        let thread_live = live.clone();
        std::thread::Builder::new()
            .name("tur-lane".into())
            .spawn(move || {
                lane_main(lane_driver, rx, thread_shared, thread_live);
            })
            .expect("failed to spawn tur lane thread");
        Self { tx, shared, live }
    }

    fn spawn_app(&self, factory: WorkerFactory) -> WorkerHandle {
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        if let Err(e) = self.tx.send(LaneMsg::SpawnApp { factory, done_tx }) {
            // Unreachable in practice: a lane in the registry always has a
            // live receiver (exit requires the registry handle to have been
            // dropped first). Kept as a defensive error path.
            tracing::error!("tur lane: app spawn delivery failed: {e:?}");
        }
        // Kick the lane so a parked thread drains the inbox promptly.
        self.shared.push(SENTINEL);
        WorkerHandle::new(Box::new(move || {
            // Join = this app's loop completion, NOT the lane thread
            // (co-tenants keep it alive).
            let _ = done_rx.recv();
        }))
    }
}

// ---------------------------------------------------------------------------
// Lane executor — runs on the lane thread
// ---------------------------------------------------------------------------

/// Cross-thread-safe wake state shared between the lane thread and wakers
/// on other threads. Only plain `u64` keys cross the boundary — never
/// `Rc`s — so pushes from any thread are sound.
#[derive(Clone)]
struct LaneShared {
    ready: Arc<Mutex<VecDeque<u64>>>,
    cv: Arc<Condvar>,
}

impl LaneShared {
    /// Push a ready key (or `SENTINEL` kick) and wake a parked lane.
    fn push(&self, key: u64) {
        let mut q = self.ready.lock().unwrap();
        q.push_back(key);
        drop(q);
        self.cv.notify_all();
    }

    fn pop_front(&self) -> Option<u64> {
        self.ready.lock().unwrap().pop_front()
    }

    /// Park until the ready queue is non-empty. The lane's only idle state
    /// — every wake-up path (waker, spawn kick, handle drop) pushes first.
    fn wait_nonempty(&self) {
        let mut q = self.ready.lock().unwrap();
        while q.is_empty() {
            q = self.cv.wait(q).unwrap();
        }
    }
}

/// Waker for lane tasks. `Send + Sync` by construction (only `u64` +
/// `Arc<Mutex<…>>`/`Arc<Condvar>` fields); safe to fire from any thread —
/// cross-thread wakes are exactly what the ready queue exists for.
struct LaneWaker {
    key: u64,
    shared: LaneShared,
}

impl Wake for LaneWaker {
    fn wake(self: Arc<Self>) {
        Self::wake_by_ref(&self);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.shared.push(self.key);
    }
}

struct TaskEntry {
    fut: Pin<Box<dyn Future<Output = ()> + 'static>>,
    /// Set only for app-loop tasks: fired on completion/panic + tracked by
    /// the lane's live-app count. Side tasks (`spawn_local`) have `None`.
    done_tx: Option<StdSender<()>>,
}

/// Lane-thread state: the task table + key allocator are lane-local
/// (`RefCell`); `shared` + `live` are the cross-thread faces.
struct LaneState {
    shared: LaneShared,
    next_key: Cell<u64>,
    tasks: RefCell<HashMap<u64, TaskEntry>>,
    live: Arc<AtomicUsize>,
}

impl LaneState {
    fn alloc_key(&self) -> u64 {
        let key = self.next_key.get();
        self.next_key.set(key + 1);
        if key == SENTINEL {
            self.alloc_key()
        } else {
            key
        }
    }

    /// Insert a task + schedule its first poll.
    fn insert_task(&self, key: u64, entry: TaskEntry) {
        self.tasks.borrow_mut().insert(key, entry);
        self.shared.push(key);
    }

    /// Deliver one app factory: build the worker view, run the factory on
    /// this thread, spawn the returned loop as a task. A panicking factory
    /// is contained (logged + `done` fired); co-tenants are unaffected.
    fn handle_spawn(self: &Rc<Self>, msg: LaneMsg, platform: &Rc<dyn WorkerSchedulerDriver>) {
        let LaneMsg::SpawnApp { factory, done_tx } = msg;
        // The worker view shares THIS lane's state — `spawn_local` from any
        // co-tenant app lands on the same task table + ready queue.
        let view = WorkerScheduler::new(Rc::new(LaneWorkerDriver {
            state: self.clone(),
            platform: platform.clone(),
        }));
        match catch_unwind(AssertUnwindSafe(|| factory(view))) {
            Ok(fut) => {
                self.live.fetch_add(1, Ordering::AcqRel);
                self.insert_task(
                    self.alloc_key(),
                    TaskEntry {
                        fut,
                        done_tx: Some(done_tx),
                    },
                );
            }
            Err(panic) => {
                tracing::error!("tur lane: app factory panicked: {panic:?}");
                let _ = done_tx.send(());
            }
        }
    }

    /// Poll one task. The entry is taken OUT of the table for the poll
    /// (so polls may call `spawn_local` freely), then re-inserted if
    /// pending. Completion and panics both finish the task (firing its
    /// `done` + decrementing `live` for app tasks).
    fn poll_task(&self, key: u64) {
        let Some(mut entry) = self.tasks.borrow_mut().remove(&key) else {
            return; // stale duplicate wake — already completed or running
        };
        let waker = Waker::from(Arc::new(LaneWaker {
            key,
            shared: self.shared.clone(),
        }));
        let mut cx = TaskContext::from_waker(&waker);
        let result = catch_unwind(AssertUnwindSafe(|| entry.fut.as_mut().poll(&mut cx)));
        match result {
            Ok(Poll::Pending) => {
                self.tasks.borrow_mut().insert(key, entry);
            }
            Ok(Poll::Ready(())) => self.finish_task(entry),
            Err(panic) => {
                tracing::error!("tur lane: task {key} panicked: {panic:?}");
                self.finish_task(entry);
            }
        }
    }

    /// Complete a finished (or panicked) task: fire its `done` signal and
    /// drop it from the table.
    fn finish_task(&self, mut entry: TaskEntry) {
        if entry.done_tx.take().is_some() {
            // App-loop task finished → one fewer live app on this lane.
            self.live.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// The worker-thread scheduling driver for lane-hosted apps. `spawn_local`
/// lands on the lane's task table; `sleep` delegates to the platform's
/// per-lane driver (self-timing timers that wake via the task `Waker`).
struct LaneWorkerDriver {
    state: Rc<LaneState>,
    platform: Rc<dyn WorkerSchedulerDriver>,
}

impl WorkerSchedulerDriver for LaneWorkerDriver {
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        track_spawn(fut, |tracked| {
            let key = self.state.alloc_key();
            self.state.insert_task(
                key,
                TaskEntry {
                    fut: tracked,
                    done_tx: None,
                },
            );
        })
    }

    fn sleep(&self, d: Duration) -> Sleep {
        self.platform.sleep(d)
    }
}

/// Lane thread entry: build the platform driver, then run the scheduling
/// loop — drain spawn inbox → poll ready tasks (bounded pass) → exit if
/// dead → park on the condvar.
fn lane_main(
    lane_driver: LaneDriverFactory,
    rx: StdReceiver<LaneMsg>,
    shared: LaneShared,
    live: Arc<AtomicUsize>,
) {
    // The platform driver is built ON the lane thread (thread-locals,
    // per-thread state).
    let platform = lane_driver();
    let state = Rc::new(LaneState {
        shared,
        next_key: Cell::new(0),
        tasks: RefCell::new(HashMap::new()),
        live,
    });
    let mut disconnected = false;
    loop {
        // 1. Drain the spawn inbox (buffered messages are still delivered
        //    after Disconnected — std mpsc guarantees — so late sends that
        //    raced a reap are never lost).
        if !disconnected {
            loop {
                match rx.try_recv() {
                    Ok(msg) => state.handle_spawn(msg, &platform),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        // 2. Poll ready tasks — one cooperative round-robin pass. The
        //    budget bounds starvation from tasks that synchronously
        //    re-wake themselves; leftovers run next iteration (after the
        //    inbox is re-checked). A sentinel is a KICK, not a task:
        //    something may have been delivered to the spawn inbox after
        //    the drain above — break out and re-drain before polling more
        //    (skipping it silently could park the lane with an undelivered
        //    Spawn — a lost wake-up).
        let mut budget = PASS_BUDGET;
        while budget > 0 {
            let Some(key) = state.shared.pop_front() else {
                break;
            };
            budget -= 1;
            if key == SENTINEL {
                break; // re-enter the outer loop → drain the inbox first
            }
            state.poll_task(key);
        }
        // 3. Exit once no more spawns can arrive and nothing is left.
        //    (Detached side tasks keep the lane alive until they finish.)
        if disconnected && state.tasks.borrow().is_empty() {
            break;
        }
        // 4. Park until a wake arrives.
        state.shared.wait_nonempty();
    }
}

// ---------------------------------------------------------------------------
// Wrapper driver — wrap any native driver wholesale
// ---------------------------------------------------------------------------

/// Convenience wrapper giving any native driver pooled
/// `spawn_worker_in`: every other method delegates to the inner driver
/// (vsync, main-thread `spawn_local`, `sleep`). Third-party embedders that
/// don't need to touch their driver's internals use this; platform crates
/// with their own driver (`tur-android`, the test harness) compose
/// [`NativeWorkerPools`] directly instead.
pub struct PooledSchedulerDriver<D: MainSchedulerDriver + 'static> {
    inner: Rc<D>,
    pools: Rc<NativeWorkerPools>,
    lane_driver: LaneDriverFactory,
}

impl<D: MainSchedulerDriver + 'static> PooledSchedulerDriver<D> {
    pub fn new(inner: Rc<D>, lane_driver: LaneDriverFactory) -> Rc<Self> {
        Rc::new(Self {
            inner,
            pools: Rc::new(NativeWorkerPools::new()),
            lane_driver,
        })
    }
}

impl<D: MainSchedulerDriver + 'static> MainSchedulerDriver for PooledSchedulerDriver<D> {
    fn spawn_worker_in(&self, pool: &WorkerPoolHandle, factory: WorkerFactory) -> WorkerHandle {
        self.pools.spawn(pool, factory, self.lane_driver.clone())
    }

    fn vsync_events(&self) -> VsyncEvents {
        self.inner.vsync_events()
    }

    fn request_vsync(&self) {
        self.inner.request_vsync();
    }

    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        self.inner.spawn_local(fut)
    }

    fn sleep(&self, d: Duration) -> Sleep {
        self.inner.sleep(d)
    }
}
