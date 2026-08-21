//! `spawn_blocking`: off-thread CPU-heavy work from worker-side tasks.
//!
//! Pins:
//! - blocking work runs OFF the worker's lane thread (distinct thread ids),
//! - the closure's return value round-trips to the awaiting task,
//! - a busy blocking task does not stall a co-tenant app sharing the same
//!   capped lane (the whole point of `spawn_blocking` on shared lanes).

use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::core::scheduler::WorkerPoolHandle;
use tur_engine::error::TurError;
use tur_engine::{TurRuntime, TurStdPlugin};
use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver};
use tur_native::NativeFontLoader;

fn eval_js(app: &Rc<tur_engine::TurApp>, source: &str) -> String {
    futures::executor::block_on(app.backend().eval_js(source))
}

/// Probe plugin: at register time spawns a worker task that runs `spin_ms`
/// of CPU work via `AsyncWorkerContext::spawn_blocking`, recording the
/// worker-lane thread id, the blocking thread id, and the computed value.
/// The test reads the shared slots afterwards.
struct BlockingProbePlugin {
    spin_ms: u64,
    lane_tid: Arc<Mutex<Option<String>>>,
    blocking_tid: Arc<Mutex<Option<String>>>,
    result: Arc<Mutex<Option<u64>>>,
}

impl Plugin for BlockingProbePlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let lane_tid = self.lane_tid.clone();
        let blocking_tid = self.blocking_tid.clone();
        let result = self.result.clone();
        let spin_ms = self.spin_ms;
        let _ = ctx.spawn_local(move |aw| async move {
            *lane_tid.lock().unwrap() = Some(format!("{:?}", std::thread::current().id()));
            let (btid, value) = aw
                .spawn_blocking(move || {
                    let start = Instant::now();
                    let mut x: u64 = 0;
                    let mut i: u64 = 0;
                    while (start.elapsed().as_millis() as u64) < spin_ms {
                        x = x.wrapping_add(i);
                        i += 1;
                    }
                    (format!("{:?}", std::thread::current().id()), x)
                })
                .await;
            *blocking_tid.lock().unwrap() = Some(btid);
            *result.lock().unwrap() = Some(value);
        });
        Ok(())
    }
}

fn build_runtime(
    driver: &Rc<TestSchedulerDriver>,
    pools: Vec<WorkerPoolHandle>,
    probe: Option<BlockingProbePlugin>,
) -> Rc<TurRuntime> {
    let mut builder = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .vsync_source(driver.vsync_source())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .plugin(TurStdPlugin);
    if let Some(probe) = probe {
        builder = builder.plugin(probe);
    }
    for pool in pools {
        builder = builder.worker_pool(pool);
    }
    builder.build().expect("runtime build")
}

/// Poll `f` until it returns `Some`, bounded by `timeout` (200ms steps).
fn wait_for<T>(timeout: Duration, mut f: impl FnMut() -> Option<T>) -> Option<T> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Some(v) = f() {
            return Some(v);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
fn blocking_work_runs_off_worker_thread_and_returns_value() {
    let driver = TestSchedulerDriver::new();
    let probe = BlockingProbePlugin {
        spin_ms: 50,
        lane_tid: Arc::new(Mutex::new(None)),
        blocking_tid: Arc::new(Mutex::new(None)),
        result: Arc::new(Mutex::new(None)),
    };
    let lane_tid = probe.lane_tid.clone();
    let blocking_tid = probe.blocking_tid.clone();
    let result = probe.result.clone();

    let pool = WorkerPoolHandle::new("offthread", usize::MAX);
    let runtime = build_runtime(&driver, vec![pool.clone()], Some(probe));
    let (_app, _looper) = runtime
        .app_builder()
        .worker_pool(pool)
        .build_headless((0.0, 0.0))
        .expect("headless app build");

    let (lane, blocking, value) = wait_for(Duration::from_secs(5), || {
        let l = lane_tid.lock().unwrap().clone()?;
        let b = blocking_tid.lock().unwrap().clone()?;
        let v = *result.lock().unwrap();
        Some((l, b, v))
    })
    .expect("blocking task settles within 5s");

    assert_ne!(
        lane, blocking,
        "spawn_blocking work must run off the worker's lane thread"
    );
    assert!(value.is_some(), "typed result must round-trip");
}

#[test]
fn blocking_work_does_not_stall_lane_cotenants() {
    let driver = TestSchedulerDriver::new();
    // One lane, two apps: A runs a 800ms blocking spin off-thread; B must
    // answer RPCs on the shared lane while A's work is in flight.
    let probe = BlockingProbePlugin {
        spin_ms: 800,
        lane_tid: Arc::new(Mutex::new(None)),
        blocking_tid: Arc::new(Mutex::new(None)),
        result: Arc::new(Mutex::new(None)),
    };
    let settled = probe.result.clone();

    let pool = WorkerPoolHandle::new("shared", 1);
    let runtime = build_runtime(&driver, vec![pool.clone()], Some(probe));

    let (_app_a, _looper_a) = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .build_headless((0.0, 0.0))
        .expect("app A build");
    let (app_b, _looper_b) = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .build_headless((0.0, 0.0))
        .expect("app B build");

    // B's round-trip must complete well inside A's remaining spin budget.
    let start = Instant::now();
    assert_eq!(eval_js(&app_b, "6 * 7"), "42");
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "co-tenant RPC stalled {elapsed:?} — blocking work leaked onto the lane"
    );
    assert!(
        settled.lock().unwrap().is_none(),
        "A's blocking work should still be running when B answered"
    );

    // A's task eventually settles (blocking thread finishes → lane task
    // wakes → slots fill).
    let done = wait_for(Duration::from_secs(5), || *settled.lock().unwrap()).is_some();
    assert!(done, "blocking task settles after the spin completes");
}
