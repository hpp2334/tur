//! Native worker-pool executor: capped shared "lane" threads.
//!
//! Implements the pool side of
//! [`WorkerSpawner`](tur_engine::core::scheduler::WorkerSpawner) for native
//! platforms. Embedders construct one [`NativeWorkerPools`] with their
//! platform's timer ([`LaneTimer`]) and pass it to the runtime builder as
//! the worker host:
//!
//! ```no_run
//! # use std::rc::Rc;
//! # use std::sync::Arc;
//! # use std::time::Duration;
//! # use tur_engine::core::scheduler::{Sleep, WorkerSpawner};
//! # use tur_native::worker_pool::{LaneTimer, NativeWorkerPools};
//! # struct MyTimer;
//! # impl LaneTimer for MyTimer {
//! #     fn sleep(&self, _: Duration) -> Sleep { Sleep(Box::pin(std::future::pending())) }
//! # }
//! // The factory must be Send + Sync (it crosses into lane threads); each
//! // call mints a fresh per-lane timer.
//! let pools = Rc::new(NativeWorkerPools::with_timer(Arc::new(|| Rc::new(MyTimer) as _)));
//! // TurRuntime::builder().worker_spawner(pools)…
//! # let _pools: Rc<NativeWorkerPools> = pools;
//! ```
//!
//! ## Model
//!
//! Each pool owns at most `max_workers` **lane** OS threads ("tur-lane").
//! App assignment is grow-to-cap-then-least-loaded: the first
//! `max_workers` apps each get a fresh lane; later apps share the
//! least-loaded existing lane. Because engine app state (`boa::Context`,
//! `Rc`s) is `!Send`, each app's loop future is pinned to exactly one lane
//! for its entire lifetime — "sharing" means multiple app loops
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
//!   senders on other threads, `spawn_blocking` completion threads,
//!   timer threads) are sound. This mirrors the wasm executor's
//!   `NoopWaker` thread-id discipline.
//! - Idle: the lane parks on the condvar when the queue is empty; every
//!   wake-up path (task waker, spawn delivery, lane-handle drop) pushes a
//!   sentinel key + `notify_all`.
//! - `sleep` is **not** reimplemented: it delegates to the platform's
//!   [`LaneTimer`] (self-timing: tokio timers, virtual test clock) whose
//!   completions wake the task via its `Waker` — which lands back in the
//!   ready queue.
//! - `spawn_blocking` runs the work on a dedicated short-lived OS thread;
//!   its completion fires a oneshot whose receiver waker re-queues the
//!   awaiting task (native's honest off-loop offload).
//! - Panic containment: the app entry call and every task poll run under
//!   `catch_unwind`; a panicking app is removed (its `done` signal fires)
//!   while co-tenant apps on the same lane keep running.
//!
//! ## Lifecycle
//!
//! A lane exits when its spawn inbox is disconnected (all senders dropped
//! — the registry reaps dead lanes lazily at the next assignment) **and**
//! its task table is empty. [`WorkerTicket::join`] blocks on that app's
//! own loop completion (not the lane thread), so several apps on one lane
//! can be joined independently. [`NativeWorkerPools`] itself is
//! main-thread only (`RefCell` registry) — `spawn_worker` is only ever
//! called from `app_builder().build(...)` on the main thread.
//!
//! ## Readiness
//!
//! `spawn_worker` blocks until the entry's synchronous prologue (backend
//! construction + plugin `register`) completed — a `started_tx` handshake
//! per [`LaneMsg::SpawnApp`] — so the engine's
//! `app_builder().build(...)` returning guarantees the worker's
//! plugin-level side effects are observable. A panicking prologue drops
//! the sender mid-unwind, failing the wait loudly.
//!
//! ## Fairness tradeoff
//!
//! Apps sharing a lane run cooperatively: a long synchronous JS flush in
//! one app stalls its lane-mates until it yields (awaits the next worker
//! message). That intra-lane coupling is the accepted boundary — the
//! guarantee pools provide is *between* pools (a busy `daemon` pool never
//! stalls a `ui` pool). CPU-heavy work should use
//! [`WorkerExecutor::spawn_blocking`](tur_engine::core::scheduler::WorkerExecutor::spawn_blocking)
//! to get off the lane entirely.

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
    BlockingSpawn, BlockingWork, Sleep, TaskHandle, WorkerContext, WorkerEntry, WorkerExecutor,
    WorkerPoolHandle, WorkerSpawner, WorkerTicket, track_spawn,
};

/// The platform's per-lane timer backend — the only platform seam a lane
/// needs besides hosting itself. Sleep-only by design: everything else on
/// a lane (task spawning, blocking offload) is provided by the lane
/// executor itself, so platforms implement exactly the piece they own
/// (tokio timers on Android, the virtual clock in tests).
pub trait LaneTimer: 'static {
    fn sleep(&self, d: Duration) -> Sleep;
}

/// Builds the per-lane [`LaneTimer`]. The closure **runs on the fresh
/// lane thread** (never on main), so it may only capture `Send + Sync`
/// state (e.g. an `Arc<tokio Handle>` or the shared virtual-test-clock).
pub type LaneTimerFactory = Arc<dyn Fn() -> Rc<dyn LaneTimer> + Send + Sync>;

/// Sentinel ready-queue key: a "something happened" kick (spawn delivered,
/// lane handle dropped). Never a real task key — allocation skips it.
const SENTINEL: u64 = u64::MAX;

/// How many ready-task polls one scheduling pass may run before
/// re-checking the spawn inbox. Bounds starvation when a task synchronously
/// re-wakes itself; normal passes finish far earlier (empty queue).
const PASS_BUDGET: u32 = 128;

// ---------------------------------------------------------------------------
// Registry — main-thread object used as the runtime's WorkerSpawner
// ---------------------------------------------------------------------------

/// Registry of worker pools → lane threads, implementing
/// [`WorkerSpawner`] for native platforms. Main-thread only (the registry
/// is a `RefCell`; `spawn_worker` is called exclusively from
/// `app_builder().build(...)`, which the engine invokes on the main
/// thread).
pub struct NativeWorkerPools {
    pools: RefCell<Vec<PoolEntry>>,
    timer_factory: LaneTimerFactory,
}

struct PoolEntry {
    handle: WorkerPoolHandle,
    lanes: Vec<LaneHandle>,
}

impl NativeWorkerPools {
    /// Construct with the platform's per-lane timer factory.
    pub fn with_timer(timer_factory: LaneTimerFactory) -> Self {
        Self {
            pools: RefCell::new(Vec::new()),
            timer_factory,
        }
    }

    /// Host one app loop in `pool`: pick the least-loaded live lane, or
    /// grow a fresh one while the pool is under its `max_workers` cap.
    /// The entry runs on the lane thread and returns the app's loop
    /// future; the returned [`WorkerTicket`] joins **that app's loop**
    /// (not the lane thread).
    ///
    /// A pool unseen by this registry is hosted on demand (fresh entry,
    /// zero lanes) — pool registration/identity was already validated by
    /// the engine (`TurAppBuilder` rejects unregistered handles), so this
    /// registry is purely a hosting detail.
    fn spawn(&self, pool: &WorkerPoolHandle, entry: WorkerEntry) -> WorkerTicket {
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
            if entry.lanes.len() < pool.max_workers() {
                // Grow: first apps each get a fresh lane (max parallelism).
                let lane = LaneHandle::spawn(self.timer_factory.clone());
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
        lane.spawn_app(entry)
    }
}

impl WorkerSpawner for NativeWorkerPools {
    fn spawn_worker(&self, pool: &WorkerPoolHandle, entry: WorkerEntry) -> WorkerTicket {
        self.spawn(pool, entry)
    }
}

// ---------------------------------------------------------------------------
// Lane handle — host-side handle to one lane thread
// ---------------------------------------------------------------------------

enum LaneMsg {
    /// Deliver an app's worker entry.
    ///
    /// - `started_tx` fires when the entry's synchronous prologue (backend
    ///   construction + plugin `register`) completed — the host side blocks
    ///   on it so `spawn_worker` returning guarantees the app is
    ///   observable.
    /// - `done_tx` fires when that app's loop future completes (or the
    ///   entry itself panics).
    SpawnApp {
        entry: WorkerEntry,
        started_tx: StdSender<()>,
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
    fn spawn(timer_factory: LaneTimerFactory) -> Self {
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
                lane_main(timer_factory, rx, thread_shared, thread_live);
            })
            .expect("failed to spawn tur lane thread");
        Self { tx, shared, live }
    }

    fn spawn_app(&self, entry: WorkerEntry) -> WorkerTicket {
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
        // Count the app BEFORE delivery: the lane thread adopts the entry
        // asynchronously, so a lane-side increment would leave a window
        // where `live == 0` for a lane with an in-flight spawn — and the
        // registry's reap-on-assign would discard it and grow past the
        // pool's cap. Main-side counting makes the invariant
        // timing-independent; the lane fires the matching decrement on
        // entry panic or loop completion.
        self.live.fetch_add(1, Ordering::AcqRel);
        if let Err(e) = self.tx.send(LaneMsg::SpawnApp {
            entry,
            started_tx,
            done_tx,
        }) {
            // Unreachable in practice: a lane in the registry always has a
            // live receiver (exit requires the registry handle to have been
            // dropped first). Kept as a defensive error path — undo the
            // count so `live` stays balanced.
            tracing::error!("tur lane: app spawn delivery failed: {e:?}");
            self.live.fetch_sub(1, Ordering::AcqRel);
        }
        // Kick the lane so a parked thread drains the inbox promptly.
        self.shared.push(SENTINEL);
        // Synchronously wait for the entry's prologue (backend construction
        // + plugin `register`): `spawn_worker` returning must guarantee the
        // worker's plugin-level side effects are observable. A prologue
        // panic drops `started_tx` mid-unwind, so `recv()` errs — matching
        // the historical "worker died during backend_factory" failure.
        started_rx
            .recv()
            .expect("tur lane died during app entry prologue");
        WorkerTicket::new(Box::new(move || {
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

    /// Deliver one app entry: build the worker view, run the entry on
    /// this thread, spawn the returned loop as a task. A panicking entry
    /// is contained (logged + `done` fired); co-tenants are unaffected.
    fn handle_spawn(self: &Rc<Self>, msg: LaneMsg, timer: &Rc<dyn LaneTimer>) {
        let LaneMsg::SpawnApp {
            entry,
            started_tx,
            done_tx,
        } = msg;
        // The worker view shares THIS lane's state — `spawn_local` from
        // any co-tenant app lands on the same task table + ready queue.
        let view = WorkerContext::new(Rc::new(LaneWorkerExecutor {
            state: self.clone(),
            timer: timer.clone(),
        }));
        match catch_unwind(AssertUnwindSafe(|| entry(view))) {
            Ok(fut) => {
                // Entry prologue completed — release the host-side
                // readiness wait (`spawn_worker`'s blocking recv).
                let _ = started_tx.send(());
                // `live` was already incremented on the main side at
                // delivery (see `LaneHandle::spawn_app`); this task's
                // completion fires the matching decrement in
                // `finish_task`.
                self.insert_task(
                    self.alloc_key(),
                    TaskEntry {
                        fut,
                        done_tx: Some(done_tx),
                    },
                );
            }
            Err(panic) => {
                tracing::error!("tur lane: app entry panicked: {panic:?}");
                // The entry never became a task, so `finish_task` will not
                // run: undo the host-side delivery count here. The panic
                // unwound past `started_tx`'s send, so the host side's
                // readiness wait fails loudly instead of observing a
                // half-built app.
                self.live.fetch_sub(1, Ordering::AcqRel);
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
                tracing::error!("tur lane: task panicked: {panic:?}");
                self.finish_task(entry);
            }
        }
    }

    fn finish_task(&self, mut entry: TaskEntry) {
        if entry.done_tx.take().is_some() {
            // App-loop task finished → one fewer live app on this lane.
            self.live.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// The lane's [`WorkerExecutor`]: `spawn_local` lands on the lane's task
/// table; `sleep` delegates to the platform's [`LaneTimer`] (self-timing,
/// wakes via the task `Waker`); `spawn_blocking` offloads to a dedicated
/// OS thread whose completion re-queues the awaiting task.
struct LaneWorkerExecutor {
    state: Rc<LaneState>,
    timer: Rc<dyn LaneTimer>,
}

impl WorkerExecutor for LaneWorkerExecutor {
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

    fn spawn_blocking(&self, work: BlockingWork) -> BlockingSpawn {
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        // Stash the work in a shared slot so a failed thread spawn can
        // recover it (the closure is dropped unrun on `Err` — with the
        // Box captured by move, the value would be lost otherwise).
        let slot = Arc::new(Mutex::new(Some(work)));
        let slot_for_thread = slot.clone();
        let spawned = std::thread::Builder::new()
            .name("tur-blocking".into())
            .spawn(move || {
                let work = slot_for_thread.lock().unwrap().take();
                // Contain a panicking closure: log + drop `tx` so the
                // awaiting task's `rx` cancels (its panic message
                // surfaces there, contained to that task).
                let panicked = catch_unwind(AssertUnwindSafe(|| {
                    if let Some(work) = work {
                        work();
                    }
                }))
                .is_err();
                if panicked {
                    tracing::error!("tur blocking task panicked");
                } else {
                    let _ = tx.send(());
                }
            });
        match spawned {
            Ok(_join) => BlockingSpawn(Box::pin(async move {
                if rx.await.is_err() {
                    panic!("spawn_blocking work panicked or was dropped");
                }
            })),
            Err(e) => {
                // Thread spawn failed (resource exhaustion): fall back to
                // running inline on this lane — degraded but correct.
                tracing::error!("tur blocking thread spawn failed ({e}); running inline");
                let work = slot.lock().unwrap().take();
                BlockingSpawn(Box::pin(async move {
                    if let Some(work) = work {
                        work();
                    }
                }))
            }
        }
    }

    fn sleep(&self, d: Duration) -> Sleep {
        self.timer.sleep(d)
    }
}

/// Lane thread entry: build the platform timer, then run the scheduling
/// loop — drain spawn inbox → poll ready tasks (bounded pass) → exit if
/// dead → park on the condvar.
fn lane_main(
    timer_factory: LaneTimerFactory,
    rx: StdReceiver<LaneMsg>,
    shared: LaneShared,
    live: Arc<AtomicUsize>,
) {
    // The platform timer is built ON the lane thread (thread-locals,
    // per-thread state).
    let timer = timer_factory();
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
                    Ok(msg) => state.handle_spawn(msg, &timer),
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
