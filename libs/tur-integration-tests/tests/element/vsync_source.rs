//! Per-instance vsync sources: each app's shell carries its own frame
//! clock and hands it to the engine at construction (`Shell::take_vsync`,
//! the Android per-`FrameLoop` pattern). Frames must flow from the
//! source the shell supplied — not from any runtime-level default.

use std::rc::Rc;

use futures::StreamExt;
use futures::channel::mpsc;
use tur_engine::core::scheduler::{VsyncSource, WorkerPoolHandle};
use tur_engine::core::shell::TextInputState;
use tur_engine::{Cursor, Shell, TurRuntime, TurStdPlugin};
use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver, TestVsyncSource};
use tur_native::NativeFontLoader;

/// A shell whose frame clock is the caller's choice — the shape Android
/// uses (`AndroidShell` carrying a Choreographer-bound source).
struct CadenceShell {
    vsync: Option<Rc<dyn VsyncSource>>,
}

impl Shell for CadenceShell {
    fn set_cursor(&mut self, _cursor: Cursor) {}
    fn request_text_input(&mut self, _state: TextInputState) {}
    fn take_vsync(&mut self) -> Option<Rc<dyn VsyncSource>> {
        self.vsync.take()
    }
}

#[test]
fn per_instance_vsync_source_drives_frames() {
    let driver = TestSchedulerDriver::new();
    let pool = WorkerPoolHandle::new("swap", usize::MAX);
    let runtime = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .worker_pool(pool.clone())
        .plugin(TurStdPlugin)
        .build()
        .expect("runtime build");

    // The shell carries a brand-new cadence of its own (NOT the driver's
    // shared source) — the engine takes it at construction.
    let fresh = TestVsyncSource::new();
    let (_app, mut looper) = runtime
        .app_builder()
        .worker_pool(pool)
        .shell(Box::new(CadenceShell {
            vsync: Some(fresh.clone()),
        }))
        .build_headless((0.0, 0.0))
        .expect("headless app build");

    let (frame_tx, mut frame_rx) = mpsc::unbounded();
    looper.set_after_frame_hook(Some(Rc::new(move |o| {
        let _ = frame_tx.unbounded_send(o);
    })));

    driver.spawn_local(Box::pin(looper.run()));

    // Frames flow from the shell-supplied source — twice, proving cadence
    // continuity rather than a bootstrap fluke.
    fresh.fire_vsync();
    let _first = driver
        .block_on(frame_rx.next())
        .expect("frame arrives via the shell-supplied vsync source");
    fresh.fire_vsync();
    let _second = driver
        .block_on(frame_rx.next())
        .expect("second frame arrives via the shell-supplied vsync source");
}

#[test]
fn a_shell_that_hands_back_no_vsync_fails_the_build() {
    let driver = TestSchedulerDriver::new();
    let pool = WorkerPoolHandle::new("novsync", usize::MAX);
    let runtime = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .worker_pool(pool.clone())
        .plugin(TurStdPlugin)
        .build()
        .expect("runtime build");

    // (The Ok type `(Rc<TurApp>, TurAppLooper)` isn't `Debug`, so no
    // `expect_err` — match instead.)
    let err = match runtime
        .app_builder()
        .worker_pool(pool)
        .shell(Box::new(CadenceShell { vsync: None }))
        .build_headless((0.0, 0.0))
    {
        Ok(_) => panic!("build must fail when the shell hands back no vsync"),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("vsync"),
        "error should name the vsync obligation, got: {err}"
    );
}
