//! The module lifecycle contract at **instance destroy**: `TurApp::destroy`
//! sends the worker's `Destroy` message, whose dispatch runs the loaded
//! module's `start()`-returned cleanup (best-effort) before the worker
//! exits. Observable: a cleanup that registers an SVG image resource ships
//! one `HostMsg::UploadImage` to the host *synchronously during teardown* —
//! the host-side image map gains the entry even though no further frames
//! are flushed. (The `worker_loop` used to intercept `Destroy` before the
//! dispatch arm that runs teardown, making destroy-time cleanup dead code.)

use std::rc::Rc;
use std::time::Duration;

use futures::executor::block_on;
use tur_engine::TurRuntime;
use tur_engine::TurStdPlugin;
use tur_engine::core::scheduler::WorkerPoolHandle;
use tur_engine::renderer::noop::NoopRenderer;
use tur_integration_tests::{TestSchedulerDriver, TestShell};
use tur_native::NativeFontLoader;

/// The module under test: `start()` returns a cleanup that registers an SVG
/// resource — a synchronous host-visible effect (one `UploadImage` ship).
const SOURCE: &str = r#"
import { createSvgResource } from "tur:std";
export function start() {
  return () => {
    createSvgResource(
      '<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">' +
      '<rect width="4" height="4" fill="red"/></svg>'
    );
  };
}
"#;

#[test]
fn module_cleanup_runs_at_destroy() {
    let driver = TestSchedulerDriver::new();
    let pool = WorkerPoolHandle::new("destroy-cleanup", usize::MAX);
    let runtime = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(
            tur_integration_tests::MutexFixedClock::new(0),
        ))
        .worker_pool(pool.clone())
        .plugin(TurStdPlugin)
        .build()
        .expect("runtime build");

    let (app, looper) = runtime
        .app_builder()
        .worker_pool(pool)
        .renderer(Box::new(NoopRenderer::new()), (64.0, 64.0), 1.0)
        .shell(TestShell::new(driver.vsync_source()))
        .build()
        .expect("app build");
    let app = Rc::new(app);
    driver.spawn_local(Box::pin(looper.run()));

    block_on(app.load_module(SOURCE)).expect("module load");
    assert_eq!(
        app.image_resource_count(),
        0,
        "no image resources before destroy (the module body registers none)"
    );

    // Drive the loop BEFORE destroy (mirroring the production embedders,
    // where the loop is live when destroy lands): consume the vsync source's
    // bootstrap tick and settle the initial frame, so the looper is parked
    // cleanly on both streams rather than carrying a stale tick that would
    // trip the destroyed-check before draining the teardown's messages.
    driver.fire_vsync();
    driver.block_on(async {
        tokio::time::sleep(Duration::from_millis(50)).await;
    });

    // Destroy: fire-and-forget. The worker's Destroy dispatch must run the
    // module's cleanup, whose `createSvgResource` ships one UploadImage to
    // the host — drive the host-side loop (the spawned looper task) until it
    // lands (bounded; the ship is synchronous in the teardown path).
    app.destroy();
    let app_for_poll = app.clone();
    let cleanup_observed = driver.block_on(async move {
        for _ in 0..2_000 {
            if app_for_poll.image_resource_count() >= 1 {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        false
    });
    assert!(
        cleanup_observed,
        "module cleanup did not run at destroy — no UploadImage shipped"
    );
}
