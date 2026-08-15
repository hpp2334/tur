//! Test-harness scheduling objects.
//!
//! Three single-role implementations for the integration tests:
//! - [`TestVsyncSource`] — manual vsync channel
//!   ([`TurApp::set_vsync_source`](tur_engine::TurApp::set_vsync_source)
//!   accepts one; the harness fires it per driven frame).
//! - [`TestMainLoop`] — main-thread task spawner backed by a thread-local
//!   tokio `LocalSet` (drives the engine's `run_loop` + the engine's
//!   main-thread drain).
//! - Worker hosting comes from
//!   [`tur_native::worker_pool::NativeWorkerPools`] (the shared native
//!   lane executor) with the virtual clock as its [`LaneTimer`] — pools
//!   registered on the runtime are hosted on "tur-lane" threads,
//!   cooperatively scheduled when a pool's cap forces sharing, with
//!   dedicated-thread `spawn_blocking` offload.
//!
//! [`TestSchedulerDriver`] bundles the three (+ the shared virtual clock)
//! as plain fields — harness ergonomics only; the engine always sees the
//! single-role objects.
//!
//! **Virtual clock**: sleep futures register deadlines against a shared
//! virtual clock. The test harness calls
//! [`TestSchedulerDriver::advance`] alongside `self.clock.forward()` to
//! advance both the boa clock + the scheduler's virtual clock. Sleep
//! wakers fire when the virtual clock reaches their deadline; on a lane
//! thread the waker lands in the lane's ready queue (see
//! `tur_native::worker_pool`).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll, Waker};
use std::time::Duration;

use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime};
use tokio::task::LocalSet;
use tur_native::worker_pool::{LaneTimer, NativeWorkerPools};

use tur_engine::core::scheduler::{MainLoop, Sleep, TaskHandle, VsyncEvents, VsyncSource};

/// Shared virtual clock state. The test harness holds a clone + advances
/// it via [`VirtualClock::advance`]; sleep futures register deadlines +
/// wakers. When the clock advances past a deadline, the waker fires.
#[derive(Default)]
struct VirtualClock {
    /// Virtual "now" in milliseconds.
    now_ms: u64,
    /// Pending timers: `deadline_ms → Vec<Waker>`. Sorted by deadline so
    /// [`VirtualClock::advance`] can fire them in order.
    timers: BTreeMap<u64, Vec<Waker>>,
}

impl VirtualClock {
    fn register_sleep(&self, d: Duration) -> u64 {
        self.now_ms + d.as_millis() as u64
    }

    fn register_waker(&mut self, deadline_ms: u64, waker: Waker) {
        self.timers.entry(deadline_ms).or_default().push(waker);
    }

    fn is_due(&self, deadline_ms: u64) -> bool {
        self.now_ms >= deadline_ms
    }

    fn advance(&mut self, ms: u64) {
        self.now_ms += ms;
        let due: Vec<u64> = self.timers.range(..=self.now_ms).map(|(k, _)| *k).collect();
        for deadline in due {
            if let Some(wakers) = self.timers.remove(&deadline) {
                for w in wakers {
                    w.wake();
                }
            }
        }
    }
}

thread_local! {
    static CURRENT_EXEC: RefCell<Option<(Rc<Runtime>, Rc<LocalSet>)>> =
        const { RefCell::new(None) };
    static CURRENT_CLOCK: RefCell<Option<Arc<Mutex<VirtualClock>>>> =
        const { RefCell::new(None) };
}

fn init_thread_exec(clock: Arc<Mutex<VirtualClock>>) {
    CURRENT_EXEC.with(|c| {
        if c.borrow().is_some() {
            return;
        }
        let rt = TokioRuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");
        *c.borrow_mut() = Some((Rc::new(rt), Rc::new(LocalSet::new())));
    });
    CURRENT_CLOCK.with(|c| *c.borrow_mut() = Some(clock));
}

/// Generic `block_on` that drives the current thread's `LocalSet` (and all
/// `spawn_local`'d tasks on it — e.g. the engine's `run_loop`) until `fut`
/// completes. Unlike `futures::executor::block_on`, this advances the
/// `LocalSet`, which is required whenever the waited result is produced by a
/// spawned task rather than directly by the worker thread.
pub(crate) fn block_on_on_current_thread_typed<F: Future>(fut: F) -> F::Output {
    let (rt, local) = CURRENT_EXEC.with(|c| {
        let guard = c.borrow();
        let Some((rt, local)) = guard.as_ref() else {
            panic!("block_on called with no LocalSet on current thread");
        };
        (rt.clone(), local.clone())
    });
    let rt_ref = &*rt;
    local.block_on(rt_ref, fut)
}

/// Construct a Sleep future backed by the shared virtual clock.
fn virtual_sleep(d: Duration) -> Sleep {
    let clock = CURRENT_CLOCK.with(|c| {
        c.borrow()
            .clone()
            .expect("virtual_sleep called with no VirtualClock on current thread")
    });
    let deadline_ms = clock.lock().unwrap().register_sleep(d);
    Sleep(Box::pin(VirtualSleepFuture {
        clock,
        deadline_ms,
        registered: false,
    }))
}

struct VirtualSleepFuture {
    clock: Arc<Mutex<VirtualClock>>,
    deadline_ms: u64,
    registered: bool,
}

impl Future for VirtualSleepFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<()> {
        let clock = self.clock.lock().unwrap();
        if clock.is_due(self.deadline_ms) {
            return Poll::Ready(());
        }
        if !self.registered {
            drop(clock);
            self.clock
                .lock()
                .unwrap()
                .register_waker(self.deadline_ms, cx.waker().clone());
            self.registered = true;
        }
        Poll::Pending
    }
}

/// Lane timer backed by the shared virtual clock. Published on each lane
/// thread by the timer factory below, so worker-side `sleep` futures
/// register against the clock the harness advances.
struct VirtualClockTimer;

impl LaneTimer for VirtualClockTimer {
    fn sleep(&self, d: Duration) -> Sleep {
        virtual_sleep(d)
    }
}

// ---------------------------------------------------------------------------
// Single-role implementations
// ---------------------------------------------------------------------------

/// Manual vsync source: [`TestVsyncSource::fire_vsync`] pushes one tick
/// into every subscribed channel. The harness (or a test) fires it once
/// per driven frame.
pub struct TestVsyncSource {
    vsync_txs: Mutex<Vec<futures::channel::mpsc::UnboundedSender<()>>>,
}

impl TestVsyncSource {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            vsync_txs: Mutex::new(Vec::new()),
        })
    }

    /// Push one vsync tick into every subscriber.
    pub fn fire_vsync(&self) {
        for tx in self.vsync_txs.lock().unwrap().iter() {
            let _ = tx.unbounded_send(());
        }
    }
}

impl VsyncSource for TestVsyncSource {
    fn subscribe(&self) -> VsyncEvents {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        // Unconditionally deliver one bootstrap tick to each new
        // subscriber. `run_loop` subscribes on its first poll (which
        // happens during the first `block_on`, AFTER any `fire_vsync`),
        // so a fire-before-subscribe would otherwise be lost. This
        // per-subscriber bootstrap tick makes the first frame reachable
        // for every run_loop — including the multi-instance case where
        // several run_loops share one source.
        let _ = tx.unbounded_send(());
        self.vsync_txs.lock().unwrap().push(tx);
        VsyncEvents(rx)
    }

    fn request_frame(&self) {
        // Manual cadence: frames advance only when the harness fires.
    }
}

/// Main-thread task spawner backed by the current thread's `LocalSet`
/// (created by [`TestSchedulerDriver::new`]). Drives the engine's
/// `run_loop` futures + the engine's main-thread drain.
pub struct TestMainLoop;

impl MainLoop for TestMainLoop {
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        let local = CURRENT_EXEC.with(|c| {
            c.borrow()
                .as_ref()
                .map(|(_, local)| local.clone())
                .expect("spawn_local called with no LocalSet on current thread")
        });
        tur_engine::core::scheduler::track_spawn(fut, |f| {
            local.spawn_local(f);
        })
    }
}

// ---------------------------------------------------------------------------
// Facade — harness ergonomics
// ---------------------------------------------------------------------------

/// Bundles the harness's three scheduling objects + the shared virtual
/// clock. Hand the single-role pieces to the runtime builder:
///
/// ```text
/// let driver = TestSchedulerDriver::new();
/// TurRuntime::builder()
///     .worker_host(driver.worker_host())
///     .vsync_source(driver.vsync_source())
///     .main_loop(driver.main_loop())
///     …
/// ```
pub struct TestSchedulerDriver {
    vsync: Rc<TestVsyncSource>,
    clock: Arc<Mutex<VirtualClock>>,
    /// Native lane-pool registry backing worker hosting (main-thread
    /// only — spawns happen from `app_builder().build()` on main).
    pools: Rc<NativeWorkerPools>,
}

impl TestSchedulerDriver {
    pub fn new() -> Rc<Self> {
        let clock = Arc::new(Mutex::new(VirtualClock::default()));
        init_thread_exec(clock.clone());
        // Publish the shared virtual clock on every lane thread (so
        // worker-side sleeps register against it) + serve `sleep` from it.
        let clock_for_factory = clock.clone();
        let pools = Rc::new(NativeWorkerPools::with_timer(Arc::new(move || {
            CURRENT_CLOCK.with(|c| *c.borrow_mut() = Some(clock_for_factory.clone()));
            Rc::new(VirtualClockTimer)
        })));
        Rc::new(Self {
            vsync: TestVsyncSource::new(),
            clock,
            pools,
        })
    }

    /// The native worker host (capped shared lane threads + virtual-clock
    /// timers + dedicated-thread `spawn_blocking`).
    pub fn worker_host(&self) -> Rc<NativeWorkerPools> {
        self.pools.clone()
    }

    /// The manual vsync source.
    pub fn vsync_source(&self) -> Rc<TestVsyncSource> {
        self.vsync.clone()
    }

    /// The main-thread task spawner.
    pub fn main_loop(&self) -> Rc<TestMainLoop> {
        Rc::new(TestMainLoop)
    }

    /// Push one vsync tick (convenience — same as
    /// `self.vsync_source().fire_vsync()`).
    pub fn fire_vsync(&self) {
        self.vsync.fire_vsync();
    }

    /// Advance the virtual clock by `ms` milliseconds, firing any due
    /// sleep wakers. The test harness calls this alongside
    /// `self.clock.forward(ms)` to advance both clocks in lockstep.
    pub fn advance(&self, ms: u64) {
        self.clock.lock().unwrap().advance(ms);
    }

    /// Drive the current thread's `LocalSet` (and all `spawn_local`'d tasks
    /// on it — the engine's `run_loop`) until `fut` completes, returning its
    /// output. Use this — not `futures::executor::block_on` — whenever the
    /// waited result is produced by a `LocalSet` task (e.g. waiting for a
    /// frame the `run_loop` emits via the `after_frame` hook).
    pub fn block_on<F: Future>(&self, fut: F) -> F::Output {
        block_on_on_current_thread_typed(fut)
    }

    /// Spawn a task on the main-thread `LocalSet` (convenience — same as
    /// `self.main_loop().spawn_local(fut)`).
    pub fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        self.main_loop().spawn_local(fut)
    }
}
