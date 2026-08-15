//! Test scheduler driver.
//!
//! Implements [`MainSchedulerDriver`] (main thread) for the integration
//! test harness. Worker spawning goes through
//! [`tur_native::worker_pool::NativeWorkerPools`] (the shared native lane
//! executor): pools registered on the runtime are hosted on "tur-lane"
//! threads, cooperatively scheduled when a pool's cap forces sharing.
//!
//! **Virtual clock**: sleep futures register deadlines against a shared
//! virtual clock ([`VirtualClock`]). The test harness calls
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
use tur_native::worker_pool::NativeWorkerPools;

use tur_engine::core::scheduler::{
    MainSchedulerDriver, Sleep, TaskHandle, VsyncEvents, WorkerFactory, WorkerHandle,
    WorkerPoolHandle, WorkerSchedulerDriver,
};

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

fn spawn_local_on_current_thread(fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
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

/// Generic `block_on` that drives the current thread's `LocalSet` (and all
/// `spawn_local`'d tasks on it — e.g. the engine's `run_loop`) until `fut`
/// completes. Unlike `futures::executor::block_on`, this advances the
/// `LocalSet`, which is required whenever the waited result is produced by a
/// spawned task rather than directly by the worker thread.
fn block_on_on_current_thread_typed<F: Future>(fut: F) -> F::Output {
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

/// Test scheduler driver.
pub struct TestSchedulerDriver {
    vsync_txs: Mutex<Vec<futures::channel::mpsc::UnboundedSender<()>>>,
    clock: Arc<Mutex<VirtualClock>>,
    /// Native lane-pool registry backing `spawn_worker_in` (main-thread
    /// only — spawns happen from `app_builder().build()` on main).
    pools: Rc<NativeWorkerPools>,
}

impl TestSchedulerDriver {
    pub fn new() -> Rc<Self> {
        let clock = Arc::new(Mutex::new(VirtualClock::default()));
        init_thread_exec(clock.clone());
        Rc::new(Self {
            vsync_txs: Mutex::new(Vec::new()),
            clock,
            pools: Rc::new(NativeWorkerPools::new()),
        })
    }

    pub fn fire_vsync(&self) {
        for tx in self.vsync_txs.lock().unwrap().iter() {
            let _ = tx.unbounded_send(());
        }
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
}

impl MainSchedulerDriver for TestSchedulerDriver {
    fn spawn_worker_in(&self, pool: &WorkerPoolHandle, factory: WorkerFactory) -> WorkerHandle {
        // Host the app on a tur-native lane thread (dedicated while the
        // pool is under its cap, shared cooperatively beyond it). The lane
        // driver publishes the shared virtual clock on the lane thread so
        // `sleep` futures register against the clock the harness advances.
        let clock = self.clock.clone();
        self.pools.spawn(
            pool,
            factory,
            Arc::new(move || {
                CURRENT_CLOCK.with(|c| *c.borrow_mut() = Some(clock.clone()));
                Rc::new(TestWorkerScheduler)
            }),
        )
    }

    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        spawn_local_on_current_thread(fut)
    }

    fn vsync_events(&self) -> VsyncEvents {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        // Unconditionally deliver one bootstrap tick to each new subscriber.
        // `run_loop` subscribes on its first poll (which happens during the
        // first `block_on`, AFTER any `fire_vsync`), so a fire-before-subscribe
        // would otherwise be lost. This per-subscriber bootstrap tick makes the
        // first frame reachable for every run_loop — including the multi-instance
        // case where several run_loops share one driver.
        let _ = tx.unbounded_send(());
        self.vsync_txs.lock().unwrap().push(tx);
        VsyncEvents(rx)
    }

    fn request_vsync(&self) {}

    fn sleep(&self, d: Duration) -> Sleep {
        virtual_sleep(d)
    }
}

/// Lane-thread scheduling driver: serves `sleep` against the shared
/// virtual clock (the lane executor provides `spawn_local` itself, so the
/// impl here is unreachable through the engine).
struct TestWorkerScheduler;

impl WorkerSchedulerDriver for TestWorkerScheduler {
    fn spawn_local(&self, _fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        unimplemented!(
            "lane spawn_local is provided by tur_native's lane executor; \
             TestWorkerScheduler only serves `sleep`"
        )
    }
    fn sleep(&self, d: Duration) -> Sleep {
        virtual_sleep(d)
    }
}
