//! Worker pools: capped shared worker threads per app group.
//!
//! Pins the pool contract end-to-end on the native lane executor
//! (`tur_native::worker_pool`): mandatory explicit assignment + engine
//! validation, one-worker-per-app when the cap allows it, cooperative
//! sharing within a capped pool, cross-pool isolation (the motivating
//! case: heavy daemon work never stalls a UI pool), and lifecycle (destroy +
//! respawn).

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use tur_engine::TurRuntime;
use tur_engine::TurStdPlugin;
use tur_engine::core::plugin::{Plugin, PluginRegisterContext};
use tur_engine::core::scheduler::WorkerPoolHandle;
use tur_engine::error::TurError;
use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver};
use tur_native::NativeFontLoader;

/// The per-instance lane-thread stamp — written by [`TidProbePlugin`] at
/// register time (which runs ON the hosting lane), read by the test thread.
type TidSlot = Arc<std::sync::Mutex<HashMap<String, String>>>;

/// The build-time per-instance key the test assigns (via
/// `TurAppBuilder::instance_data`), under which the plugin stamps the lane.
#[derive(Clone)]
struct TidKey(String);

/// Test-only plugin that stamps the worker/lane thread's id into a shared
/// slot keyed by the instance's `TidKey`, so tests can observe which thread
/// hosts each app's engine.
struct TidProbePlugin {
    slot: TidSlot,
}

impl Plugin for TidProbePlugin {
    fn register(&self, ctx: &mut PluginRegisterContext) -> Result<(), TurError> {
        let tid = format!("{:?}", std::thread::current().id());
        let key = ctx
            .instance()
            .data::<TidKey>()
            .map(|k| k.0.clone())
            .unwrap_or_default();
        self.slot.lock().unwrap().insert(key, tid);
        Ok(())
    }
}

/// Build a runtime + driver registering the given pools. Instances assign
/// one of them explicitly.
fn build_runtime(pools: Vec<WorkerPoolHandle>) -> (Rc<TurRuntime>, Rc<TestSchedulerDriver>) {
    build_runtime_probed(pools, Arc::new(std::sync::Mutex::new(HashMap::new())))
}

fn build_runtime_probed(
    pools: Vec<WorkerPoolHandle>,
    slot: TidSlot,
) -> (Rc<TurRuntime>, Rc<TestSchedulerDriver>) {
    let driver = TestSchedulerDriver::new();
    let mut builder = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .plugin(TurStdPlugin)
        .plugin(TidProbePlugin { slot });
    for pool in pools {
        builder = builder.worker_pool(pool);
    }
    let runtime = builder.build().expect("runtime build");
    (runtime, driver)
}

fn spawn_headless(
    runtime: &Rc<TurRuntime>,
    pool: &WorkerPoolHandle,
    slot: &TidSlot,
    key: &str,
) -> Rc<tur_engine::TurApp> {
    let key_string = key.to_string();
    let (app, _looper) = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .instance_data(move |cx| {
            cx.define::<TidKey>(TidKey(key_string.clone()));
        })
        .build_headless((0.0, 0.0))
        .expect("headless app build");
    // The register-time stamp may land a beat after build returns on wasm
    // lanes (native builds rendezvous in the spawn handshake) — poll.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !slot.lock().unwrap().contains_key(key) {
        if std::time::Instant::now() > deadline {
            panic!("the tid probe never stamped the slot");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app
}

fn tid_of(slot: &TidSlot, key: &str) -> String {
    slot.lock().unwrap().get(key).cloned().expect("stamped tid")
}

// ---------- Validation ------------------------------------------------------

/// Assert a builder terminal errored; return the error's message (the Ok
/// types are `Rc<TurApp>` etc. without `Debug`, so `expect_err` is out).
fn expect_err_msg<T>(result: Result<T, TurError>, what: &str) -> String {
    match result {
        Ok(_) => panic!("expected error: {what}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn missing_worker_pool_assignment_errors() {
    let pool = WorkerPoolHandle::new("p", 1);
    let (runtime, _driver) = build_runtime(vec![pool]);
    let msg = expect_err_msg(
        runtime.app_builder().build_headless((0.0, 0.0)),
        "build must require .worker_pool",
    );
    assert!(
        msg.contains(".worker_pool"),
        "error should demand .worker_pool, got: {msg}"
    );
}

#[test]
fn unregistered_pool_handle_errors() {
    let (runtime, _driver) = build_runtime(vec![WorkerPoolHandle::new("known", 1)]);
    let rogue = WorkerPoolHandle::new("rogue", 1);
    let msg = expect_err_msg(
        runtime
            .app_builder()
            .worker_pool(rogue)
            .build_headless((0.0, 0.0)),
        "unregistered pool must be rejected",
    );
    assert!(
        msg.contains("rogue") && msg.contains("not registered"),
        "error should name the unregistered pool, got: {msg}"
    );
}

#[test]
fn zero_max_workers_errors_at_runtime_build() {
    let driver = TestSchedulerDriver::new();
    let msg = expect_err_msg(
        TurRuntime::builder()
            .worker_spawner(driver.worker_spawner())
            .host_loop(driver.host_loop())
            .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
            .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
            .worker_pool(WorkerPoolHandle::new("bad", 0))
            .build(),
        "max_workers == 0 must fail build",
    );
    assert!(
        msg.contains("max_workers"),
        "error should mention max_workers, got: {msg}"
    );
}

#[test]
fn duplicate_pool_name_errors_at_runtime_build() {
    let driver = TestSchedulerDriver::new();
    let msg = expect_err_msg(
        TurRuntime::builder()
            .worker_spawner(driver.worker_spawner())
            .host_loop(driver.host_loop())
            .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
            .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
            .worker_pool(WorkerPoolHandle::new("dup", 1))
            .worker_pool(WorkerPoolHandle::new("dup", 2))
            .build(),
        "duplicate pool name must fail build",
    );
    assert!(
        msg.contains("dup"),
        "error should name the duplicate, got: {msg}"
    );
}

// ---------- Placement: grow-to-cap, then share ------------------------------

#[test]
fn uncapped_pool_gives_each_app_its_own_thread() {
    // Backward-compatible degenerate case: cap ≥ app count → one lane per
    // app (the historical one-thread-per-app behavior).
    let pool = WorkerPoolHandle::new("wide", usize::MAX);
    let slot: TidSlot = Arc::new(std::sync::Mutex::new(HashMap::new()));
    let (runtime, _driver) = build_runtime_probed(vec![pool.clone()], slot.clone());
    let _app_a = spawn_headless(&runtime, &pool, &slot, "a");
    let _app_b = spawn_headless(&runtime, &pool, &slot, "b");

    assert_ne!(
        tid_of(&slot, "a"),
        tid_of(&slot, "b"),
        "uncapped pool: each app gets its own lane thread"
    );
}

#[test]
fn capped_pool_shares_one_thread_between_apps() {
    let pool = WorkerPoolHandle::new("narrow", 1);
    let slot: TidSlot = Arc::new(std::sync::Mutex::new(HashMap::new()));
    let (runtime, _driver) = build_runtime_probed(vec![pool.clone()], slot.clone());
    let _app_a = spawn_headless(&runtime, &pool, &slot, "a");
    let _app_b = spawn_headless(&runtime, &pool, &slot, "b");

    assert_eq!(
        tid_of(&slot, "a"),
        tid_of(&slot, "b"),
        "max=1 pool: both apps share one lane thread"
    );
}

#[test]
fn capped_pool_never_exceeds_max_workers() {
    let pool = WorkerPoolHandle::new("two", 2);
    let slot: TidSlot = Arc::new(std::sync::Mutex::new(HashMap::new()));
    let (runtime, _driver) = build_runtime_probed(vec![pool.clone()], slot.clone());
    for i in 0..4 {
        spawn_headless(&runtime, &pool, &slot, &i.to_string());
    }

    let distinct: std::collections::HashSet<_> =
        (0..4).map(|i| tid_of(&slot, &i.to_string())).collect();
    assert_eq!(
        distinct.len(),
        2,
        "4 apps in a max=2 pool must land on exactly 2 threads (grow-to-cap then share), got {:?}",
        distinct
    );
}

#[test]
fn capped_pool_holds_cap_while_lane_adoption_lags() {
    // Regression: the registry used to reap lanes whose `live` count was
    // still 0 because the lane thread hadn't adopted the in-flight app
    // entry yet — under load (CI containers) a back-to-back second spawn
    // then grew a NEW lane and blew past the cap. The count is now taken
    // at delivery time on the main side, so hammering back-to-back spawns
    // must hold the cap no matter how slowly the lane thread adopts.
    let pool = WorkerPoolHandle::new("hammer", 1);
    let slot: TidSlot = Arc::new(std::sync::Mutex::new(HashMap::new()));
    let (runtime, _driver) = build_runtime_probed(vec![pool.clone()], slot.clone());
    for round in 0..12 {
        let ka = format!("r{round}a");
        let kb = format!("r{round}b");
        spawn_headless(&runtime, &pool, &slot, &ka);
        // No wait between the two builds — this is the race window.
        spawn_headless(&runtime, &pool, &slot, &kb);
        assert_eq!(
            tid_of(&slot, &ka),
            tid_of(&slot, &kb),
            "round {round}: back-to-back spawns must share the one lane"
        );
    }
}

// ---------- Cross-pool isolation (the motivation) ---------------------------

#[test]
fn heavy_daemon_work_does_not_stall_other_pools() {
    let ui = WorkerPoolHandle::new("ui", 2);
    let daemon = WorkerPoolHandle::new("daemon", 1);
    let slot: TidSlot = Arc::new(std::sync::Mutex::new(HashMap::new()));
    let (runtime, driver) = build_runtime_probed(vec![ui.clone(), daemon.clone()], slot.clone());
    let ui_app = spawn_headless(&runtime, &ui, &slot, "ui");
    let daemon_app = spawn_headless(&runtime, &daemon, &slot, "daemon");

    // Kick off a long synchronous rut busy-loop in the daemon app, driven
    // from the main-thread executor (TurApp is !Send — no OS thread can
    // hold it). The entry call monopolizes the daemon lane until it
    // finishes — exactly the workload that must not affect other pools.
    futures::executor::block_on(daemon_app.clone().load_rut_module(
        r#"
entry fn start() {
}

// A few hundred million iterations of pure rut arithmetic — long enough
// that the ui round-trips must interleave well before it finishes.
entry fn busy(_a: u64, _b: f64) {
    let mut n = 0;
    let mut i = 0;
    while (i < 200000000) {
        n += i;
        i += 1;
    }
}
"#,
    ))
    .expect("daemon module load");
    let daemon_done = Rc::new(std::cell::Cell::new(false));
    let done_for_task = daemon_done.clone();
    let (finished_tx, finished_rx) = futures::channel::oneshot::channel::<()>();
    let daemon_for_task = daemon_app.clone();
    driver.spawn_local(Box::pin(async move {
        let _ = daemon_for_task.call_rut_entry("busy", 0, 0.0).await;
        done_for_task.set(true);
        let _ = finished_tx.send(());
    }));

    // The ui app (different pool) still loads a module + answers RPCs
    // while the daemon is mid-loop. If pools were broken (both apps on one
    // thread), this block_on would queue behind the busy loop and only
    // return after it finished.
    futures::executor::block_on(ui_app.load_rut_module(
        r#"
use tur::mount;
use tur_kit::{ Mutation, Readable, Source, Text, source };


entry fn start() {
    let atom: Readable<str> = source<str>("42");
    let txt = Text().text_bound(atom).build();
    mount(txt);
}
"#,
    ))
    .expect("ui load_rut_module must complete while daemon busy-loops");
    assert!(
        !daemon_done.get(),
        "daemon busy-loop should still be running (it must outlast the ui round-trips)"
    );

    // Daemon eventually finishes and stays correct (drives the LocalSet
    // until the daemon task's completion signal fires).
    driver.block_on(finished_rx).expect("daemon task completes");
}

// ---------- Lifecycle --------------------------------------------------------

#[test]
fn destroy_pooled_app_co_tenants_and_respawn_survive() {
    let pool = WorkerPoolHandle::new("lifecycle", 1);
    let slot: TidSlot = Arc::new(std::sync::Mutex::new(HashMap::new()));
    let (runtime, _driver) = build_runtime_probed(vec![pool.clone()], slot.clone());
    let app_a = spawn_headless(&runtime, &pool, &slot, "a");
    let _app_b = spawn_headless(&runtime, &pool, &slot, "b");

    // Destroy one co-tenant, then give the lane a moment to process the
    // Destroy (its loop future completes → live count drops → the lane is
    // reaped at the next spawn).
    app_a.destroy();
    std::thread::sleep(Duration::from_millis(200));

    // A subsequent spawn into the same pool works (fresh lane after the
    // old one reaped, or reuse — either way it must build + run + register).
    spawn_headless(&runtime, &pool, &slot, "c");
}
