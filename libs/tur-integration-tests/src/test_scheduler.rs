//! Test scheduler driver.
//!
//! Implements [`MainSchedulerDriver`] (main thread) and
//! [`WorkerSchedulerDriver`] (worker thread, via [`TestWorkerScheduler`])
//! for the integration test harness. Uses real `std::thread::spawn` per
//! worker (faithful to production threading) + a per-thread
//! `tokio::task::LocalSet` for `spawn_local` / `block_on`.
//!
//! **Virtual clock**: sleep futures register deadlines against a shared
//! virtual clock ([`VirtualClock`]). The test harness calls
//! [`TestSchedulerDriver::advance`] alongside `self.clock.forward()` to
//! advance both the boa clock + the scheduler's virtual clock. Sleep
//! wakers fire when the virtual clock reaches their deadline.

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

use tur_engine::core::scheduler::{
    MainSchedulerDriver, Sleep, TaskHandle, VsyncEvents, WorkerHandle, WorkerScheduler,
    WorkerSchedulerDriver, track_spawn,
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
    track_spawn(fut, |f| {
        local.spawn_local(f);
    })
}

fn block_on_on_current_thread(fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
    block_on_on_current_thread_typed(fut);
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

/// Drive the current thread's `LocalSet` until idle so every spawned
/// task (`run_loop` futures etc.) observes closed channels, completes, and
/// DROPS while the thread's TLS is still alive. Without this, tasks sit in
/// the thread-local `LocalSet` until thread destruction — dropping
/// wgpu-backed state (render targets) after TLS teardown panics inside the
/// destructor and aborts the process. Call once at the end of a test
/// binary's `main` (see `tests/vello/main.rs`).
pub fn settle_local_tasks() {
    let Some((rt, local)) = CURRENT_EXEC.with(|c| {
        c.borrow()
            .as_ref()
            .map(|(rt, local)| (rt.clone(), local.clone()))
    }) else {
        return;
    };
    let rt_ref = &*rt;
    // Poll the set repeatedly (a yield completes immediately, so each
    // block_on is one poll round); completed run_loops release their
    // `Rc<TurApp>` clones while the thread is still alive. The worker
    // thread needs a beat to observe the closed channels and drop its
    // senders, hence the tiny sleep per round. A bound guards against a
    // task that never quiesces.
    for _ in 0..64 {
        local.block_on(rt_ref, async {
            tokio::task::yield_now().await;
        });
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
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
}

impl TestSchedulerDriver {
    pub fn new() -> Rc<Self> {
        let clock = Arc::new(Mutex::new(VirtualClock::default()));
        init_thread_exec(clock.clone());
        Rc::new(Self {
            vsync_txs: Mutex::new(Vec::new()),
            clock,
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
    fn spawn_worker(&self, factory: tur_engine::core::scheduler::WorkerFactory) -> WorkerHandle {
        let clock = self.clock.clone();
        let join = std::thread::Builder::new()
            .name("tur-test-worker".into())
            .spawn(move || {
                init_thread_exec(clock);
                let worker_view = WorkerScheduler::new(Rc::new(TestWorkerScheduler));
                let loop_fut = factory(worker_view);
                // Drive the worker's main future to completion on the test
                // executor — an infinite loop, so the thread blocks forever,
                // polling the loop + all spawn_local'd side tasks.
                block_on_on_current_thread(loop_fut);
                CURRENT_EXEC.with(|c| *c.borrow_mut() = None);
                CURRENT_CLOCK.with(|c| *c.borrow_mut() = None);
            })
            .expect("failed to spawn tur test worker thread");
        WorkerHandle::new(Box::new(move || {
            let _ = join.join();
        }))
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

struct TestWorkerScheduler;

impl WorkerSchedulerDriver for TestWorkerScheduler {
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        spawn_local_on_current_thread(fut)
    }
    fn sleep(&self, d: Duration) -> Sleep {
        virtual_sleep(d)
    }
}
