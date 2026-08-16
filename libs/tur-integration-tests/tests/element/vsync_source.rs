//! Per-instance vsync sources: `TurApp::set_vsync_source` swaps the frame
//! cadence after build (the Android per-`FrameLoop` pattern). Frames must
//! flow from the source installed at `run_loop` start.

use std::rc::Rc;

use futures::StreamExt;
use futures::channel::mpsc;
use tur_engine::core::scheduler::WorkerPoolHandle;
use tur_engine::{TurRuntime, TurStdPlugin};
use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver, TestVsyncSource};
use tur_native::NativeFontLoader;

#[test]
fn per_instance_vsync_source_drives_frames() {
    let driver = TestSchedulerDriver::new();
    let pool = WorkerPoolHandle::new("swap", usize::MAX);
    let runtime = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .vsync_source(driver.vsync_source())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .worker_pool(pool.clone())
        .plugin(TurStdPlugin)
        .build()
        .expect("runtime build");

    let app = runtime
        .app_builder()
        .worker_pool(pool)
        .build_headless((0.0, 0.0))
        .expect("headless app build");

    let (frame_tx, mut frame_rx) = mpsc::unbounded();
    app.set_after_frame_hook(Some(Rc::new(move |o| {
        let _ = frame_tx.unbounded_send(o);
    })));

    // Swap in a brand-new vsync source BEFORE the loop starts (the Android
    // pattern: install_frame_loop → set_vsync_source → run_loop).
    let fresh = TestVsyncSource::new();
    app.set_vsync_source(fresh.clone());

    driver.spawn_local(Box::pin(app.clone().run_loop()));

    // Frames flow from the swapped-in source — twice, proving cadence
    // continuity rather than a bootstrap fluke.
    fresh.fire_vsync();
    let _first = driver
        .block_on(frame_rx.next())
        .expect("frame arrives via the swapped-in vsync source");
    fresh.fire_vsync();
    let _second = driver
        .block_on(frame_rx.next())
        .expect("second frame arrives via the swapped-in vsync source");
}
