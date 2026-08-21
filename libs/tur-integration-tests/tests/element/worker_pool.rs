//! Worker pools: capped shared worker threads per app group.
//!
//! Pins the pool contract end-to-end on the native lane executor
//! (`tur_native::worker_pool`): mandatory explicit assignment + engine
//! validation, one-worker-per-app when the cap allows it, cooperative
//! sharing within a capped pool, cross-pool isolation (the motivating
//! case: heavy daemon JS never stalls a UI pool), and lifecycle (destroy +
//! respawn).

use std::rc::Rc;
use std::time::Duration;

use boa_engine::JsValue;
use boa_engine::js_string;
use tur_engine::TurRuntime;
use tur_engine::TurStdPlugin;
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::core::scheduler::WorkerPoolHandle;
use tur_engine::error::TurError;
use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver};
use tur_native::NativeFontLoader;

/// Eval a JS script on a `TurApp` (worker-thread RPC) and return the
/// result as a string.
fn eval_js(app: &Rc<tur_engine::TurApp>, source: &str) -> String {
    futures::executor::block_on(app.backend().eval_js(source))
}

/// Test-only plugin that stamps the worker/lane thread's id into a global
/// so tests can observe which thread hosts each app's engine.
struct TidProbePlugin;

impl Plugin for TidProbePlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let tid = format!("{:?}", std::thread::current().id());
        ctx.register_global("__turTid", JsValue::from(js_string!(tid.as_str())));
        Ok(())
    }
}

/// Build a runtime + driver registering the given pools. Instances assign
/// one of them explicitly.
fn build_runtime(pools: Vec<WorkerPoolHandle>) -> (Rc<TurRuntime>, Rc<TestSchedulerDriver>) {
    let driver = TestSchedulerDriver::new();
    let mut builder = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .plugin(TurStdPlugin)
        .plugin(TidProbePlugin);
    for pool in pools {
        builder = builder.worker_pool(pool);
    }
    let runtime = builder.build().expect("runtime build");
    (runtime, driver)
}

fn spawn_headless(runtime: &Rc<TurRuntime>, pool: &WorkerPoolHandle) -> Rc<tur_engine::TurApp> {
    let (app, _looper) = runtime
        .app_builder()
        .worker_pool(pool.clone())
        .build_headless((0.0, 0.0))
        .expect("headless app build");
    app
}

fn tid_of(app: &Rc<tur_engine::TurApp>) -> String {
    // Give the worker a moment to finish registering globals if the
    // build's init barrier somehow raced (it shouldn't — build blocks on
    // native init), then read the stamped tid.
    let tid = eval_js(app, "globalThis.__turTid");
    assert!(!tid.is_empty(), "__turTid should be registered");
    tid
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
    let (runtime, _driver) = build_runtime(vec![pool.clone()]);
    let app_a = spawn_headless(&runtime, &pool);
    let app_b = spawn_headless(&runtime, &pool);

    assert_ne!(
        tid_of(&app_a),
        tid_of(&app_b),
        "uncapped pool: each app gets its own lane thread"
    );

    // Both still run JS.
    eval_js(&app_a, r#"globalThis.__x = "a""#);
    eval_js(&app_b, r#"globalThis.__x = "b""#);
    assert_eq!(eval_js(&app_a, "globalThis.__x"), "a");
    assert_eq!(eval_js(&app_b, "globalThis.__x"), "b");
}

#[test]
fn capped_pool_shares_one_thread_between_apps() {
    let pool = WorkerPoolHandle::new("narrow", 1);
    let (runtime, _driver) = build_runtime(vec![pool.clone()]);
    let app_a = spawn_headless(&runtime, &pool);
    let app_b = spawn_headless(&runtime, &pool);

    assert_eq!(
        tid_of(&app_a),
        tid_of(&app_b),
        "max=1 pool: both apps share one lane thread"
    );

    // Realms stay isolated on the shared thread.
    eval_js(&app_a, r#"globalThis.__who = "A""#);
    eval_js(&app_b, r#"globalThis.__who = "B""#);
    assert_eq!(eval_js(&app_a, "globalThis.__who"), "A");
    assert_eq!(eval_js(&app_b, "globalThis.__who"), "B");
}

#[test]
fn capped_pool_never_exceeds_max_workers() {
    let pool = WorkerPoolHandle::new("two", 2);
    let (runtime, _driver) = build_runtime(vec![pool.clone()]);
    let apps: Vec<_> = (0..4).map(|_| spawn_headless(&runtime, &pool)).collect();

    let distinct: std::collections::HashSet<_> = apps.iter().map(tid_of).collect();
    assert_eq!(
        distinct.len(),
        2,
        "4 apps in a max=2 pool must land on exactly 2 threads (grow-to-cap then share), got {:?}",
        distinct
    );
    // Every app still answers RPCs.
    for (i, app) in apps.iter().enumerate() {
        assert_eq!(
            eval_js(app, &format!("{i} + 1")),
            (i + 1).to_string(),
            "app {i} alive"
        );
    }
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
    let (runtime, _driver) = build_runtime(vec![pool.clone()]);
    for round in 0..12 {
        let a = spawn_headless(&runtime, &pool);
        // No wait between the two builds — this is the race window.
        let b = spawn_headless(&runtime, &pool);
        assert_eq!(
            tid_of(&a),
            tid_of(&b),
            "round {round}: back-to-back spawns must share the one lane"
        );
    }
}

// ---------- Cross-pool isolation (the motivation) ---------------------------

#[test]
fn heavy_daemon_work_does_not_stall_other_pools() {
    let ui = WorkerPoolHandle::new("ui", 2);
    let daemon = WorkerPoolHandle::new("daemon", 1);
    let (runtime, driver) = build_runtime(vec![ui.clone(), daemon.clone()]);
    let ui_app = spawn_headless(&runtime, &ui);
    let daemon_app = spawn_headless(&runtime, &daemon);

    // Kick off a long SYNCHRONOUS JS busy-loop in the daemon app, driven
    // from the main-thread executor (TurApp is !Send — no OS thread can
    // hold it). The eval itself monopolizes the daemon lane until it
    // finishes — exactly the workload that must not affect other pools.
    let daemon_done = Rc::new(std::cell::Cell::new(false));
    let done_for_task = daemon_done.clone();
    let (finished_tx, finished_rx) = futures::channel::oneshot::channel::<()>();
    let daemon_for_task = daemon_app.clone();
    driver.spawn_local(Box::pin(async move {
        let busy = r#"
            let n = 0;
            for (let i = 0; i < 2_000_000; i++) { n += i; }
            globalThis.__daemonDone = n;
        "#
        .to_string();
        let _ = daemon_for_task.backend().eval_js(&busy).await;
        done_for_task.set(true);
        let _ = finished_tx.send(());
    }));

    // The ui app (different pool) still loads a module + answers RPCs
    // while the daemon is mid-loop. If pools were broken (both apps on one
    // thread), this block_on would queue behind the busy loop and only
    // return after it finished.
    futures::executor::block_on(ui_app.backend().load_module(
        r#"const store = createStore();

            import { createStore, source } from "tur:std";
            export function start() {
                globalThis.__uiVal = store.get(source(42));
            }
        "#,
    ))
    .expect("ui load_module must complete while daemon busy-loops");
    assert_eq!(eval_js(&ui_app, "globalThis.__uiVal"), "42");
    assert!(
        !daemon_done.get(),
        "daemon busy-loop should still be running (it must outlast the ui round-trips)"
    );

    // Daemon eventually finishes and stays correct (drives the LocalSet
    // until the daemon task's completion signal fires).
    driver.block_on(finished_rx).expect("daemon task completes");
    let done = eval_js(&daemon_app, "typeof globalThis.__daemonDone === 'number'");
    assert_eq!(done, "true", "daemon loop completed");
}

// ---------- Lifecycle --------------------------------------------------------

#[test]
fn destroy_pooled_app_co_tenants_and_respawn_survive() {
    let pool = WorkerPoolHandle::new("lifecycle", 1);
    let (runtime, _driver) = build_runtime(vec![pool.clone()]);
    let app_a = spawn_headless(&runtime, &pool);
    let app_b = spawn_headless(&runtime, &pool);

    // Destroy one co-tenant, then give the lane a moment to process the
    // Destroy (its loop future completes → live count drops → the lane is
    // reaped at the next spawn).
    app_a.destroy();
    std::thread::sleep(Duration::from_millis(200));

    // The surviving co-tenant keeps running JS on the shared lane.
    eval_js(&app_b, r#"globalThis.__alive = "yes""#);
    assert_eq!(eval_js(&app_b, "globalThis.__alive"), "yes");

    // A subsequent spawn into the same pool works (fresh lane after the
    // old one reaped, or reuse — either way it must build + run).
    let app_c = spawn_headless(&runtime, &pool);
    assert_eq!(eval_js(&app_c, "7 * 6"), "42");
}
