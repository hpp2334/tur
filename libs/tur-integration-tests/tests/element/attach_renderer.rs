//! The two-phase (initialize → attach) instance lifecycle: an instance may
//! be built **detached** (`build_headless` — no renderer) and get its
//! renderer later via [`TurApp::attach_renderer`], with
//! [`TurApp::detach_renderer`] dropping it again. While detached, every
//! render-side call (batch application, present, image upload, resize,
//! readback) must skip silently — the engine loop (JS, flushes, events)
//! keeps running. This is the seam tur-android's
//! `createInstance`/`attachInstance`/`detachInstance` ops are built on: no
//! surface work can exist for an instance that hasn't attached one.

use std::cell::RefCell;
use std::rc::Rc;

use futures::StreamExt;
use futures::channel::mpsc;
use futures::executor::block_on;
use tur_engine::TurRuntime;
use tur_engine::TurStdPlugin;
use tur_engine::core::app::FrameOutcome;
use tur_engine::core::render::{RenderCommand, Renderer};
use tur_engine::core::scheduler::WorkerPoolHandle;
use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver, TestShell};
use tur_native::NativeFontLoader;

/// A module whose tree depends on the viewport (an `Expanded` child fills
/// the remaining flex space) so a resize dirties it and flushes ship real
/// render batches.
const SOURCE: &str = r##"

use tur_kit::handles::{ mount };
use tur_kit::layout::box::{ Container };
use tur_kit::reactive::{ sv };

entry fn start() {
    let b = Container().width_height(40.0, 40.0).color(sv(0x336699FFu64));
    mount(b.build());
}
"##;

/// Records every `Renderer` call into a shared log.
struct RecordingRenderer {
    calls: Rc<RefCell<Vec<String>>>,
}

impl Renderer for RecordingRenderer {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        self.calls
            .borrow_mut()
            .push(format!("render_commands:{}", commands.len()));
    }
    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.calls.borrow_mut().push("present".into());
        Ok(())
    }
    fn resize(&mut self, logical_width: u32, logical_height: u32, dpr: f64) {
        self.calls
            .borrow_mut()
            .push(format!("resize:{logical_width}x{logical_height}@{dpr}"));
    }
    fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        self.calls.borrow_mut().push("render_to_pixels".into());
        Some(vec![1, 2, 3, 4])
    }
}

/// Drive exactly one frame (the harness `pump_one` pattern: drain stale
/// outcomes, fire vsync, await one outcome).
fn pump(
    driver: &TestSchedulerDriver,
    frame_rx: &RefCell<mpsc::UnboundedReceiver<FrameOutcome>>,
) -> FrameOutcome {
    use futures::future::FutureExt;
    while let Some(Some(_stale)) = frame_rx.borrow_mut().next().now_or_never() {}
    driver.fire_vsync();
    driver
        .block_on(frame_rx.borrow_mut().next())
        .expect("frame outcome")
}

/// Drive frames until the engine goes quiet (a resize takes one frame to
/// process + one to paint; a fixed pump count is the quiescence form for
/// this test).
fn pump_quiet(
    driver: &TestSchedulerDriver,
    frame_rx: &RefCell<mpsc::UnboundedReceiver<FrameOutcome>>,
) {
    for _ in 0..5 {
        let _ = pump(driver, frame_rx);
    }
}

#[test]
fn renderer_slot_attach_detach_cycle() {
    let driver = TestSchedulerDriver::new();
    let pool = WorkerPoolHandle::new("attach-renderer", usize::MAX);
    let runtime = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .worker_pool(pool.clone())
        .plugin(TurStdPlugin)
        .build()
        .expect("runtime build");

    // INITIALIZE: detached instance (no renderer) — JS + flushes + events
    // run, render output goes nowhere.
    let (app, mut looper) = runtime
        .app_builder()
        .worker_pool(pool)
        .shell(TestShell::new(driver.vsync_source()))
        .build_headless((64.0, 64.0))
        .expect("headless build");
    let app = Rc::new(app);
    let (frame_tx, frame_rx) = mpsc::unbounded::<FrameOutcome>();
    looper.set_after_frame_hook(Some(Rc::new(move |o| {
        let _ = frame_tx.unbounded_send(o);
    })));
    driver.spawn_local(Box::pin(looper.run()));
    let frame_rx = RefCell::new(frame_rx);

    // The module mounts + the tree paints — while detached the batches are
    // discarded (no renderer to receive them; no panic).
    block_on(app.load_rut_module(SOURCE)).expect("module load");
    pump_quiet(&driver, &frame_rx);
    assert_eq!(
        app.render_to_pixels(),
        None,
        "detached readback must be None"
    );

    // ATTACH: install a renderer; the attach resize sizes it (a different
    // size than the headless bootstrap → relayout → repaint) and the next
    // frame flows into it.
    let calls = Rc::new(RefCell::new(Vec::new()));
    app.attach_renderer(
        Box::new(RecordingRenderer {
            calls: calls.clone(),
        }),
        96,
        96,
        1.0,
    );
    pump_quiet(&driver, &frame_rx);
    {
        let log = calls.borrow();
        assert!(
            log.iter().any(|c| c == "resize:96x96@1"),
            "attach must size the renderer (got {log:?})"
        );
        assert!(
            log.iter().any(|c| c.starts_with("render_commands:")),
            "attached frames must reach the renderer (got {log:?})"
        );
        assert!(
            log.iter().any(|c| c == "present"),
            "attached frames must present (got {log:?})"
        );
    }
    // Attached readback now works.
    assert_eq!(app.render_to_pixels(), Some(vec![1, 2, 3, 4]));

    // DETACH: drop the renderer. A subsequent dirty frame (a resize event)
    // still flushes — but nothing reaches any renderer, and readback is
    // None again. Re-attach resumes rendering.
    let calls_before_detach = calls.borrow().len();
    app.detach_renderer();
    app.resize(32, 32, 1.0);
    pump_quiet(&driver, &frame_rx);
    assert_eq!(
        calls.borrow().len(),
        calls_before_detach,
        "frames while detached must not reach a renderer"
    );
    assert_eq!(app.render_to_pixels(), None, "detached readback is None");

    app.attach_renderer(
        Box::new(RecordingRenderer {
            calls: calls.clone(),
        }),
        32,
        32,
        1.0,
    );
    pump_quiet(&driver, &frame_rx);
    assert!(
        calls
            .borrow()
            .iter()
            .any(|c| c.starts_with("render_commands:")),
        "re-attached frames must reach the renderer"
    );

    // Detach is idempotent; destroy works from the detached state.
    app.detach_renderer();
    app.detach_renderer();
    app.destroy();
}

/// A dpr-only change — same logical size, different `devicePixelRatio`
/// (browser zoom on a fixed-size window, a monitor move, an OS
/// display-scale change) — must re-sync the host renderer. The engine
/// path: `ResizeSubsystem` updates `Screen.dpr` even when the logical
/// size (and thus `viewportSize$`) is unchanged, the flush still paints
/// (`mark_root_dirty` + `request_paint`), and the render commit point's
/// `sync_viewport` dedups on the full viewport triple (dpr included) so
/// the new geometry applies with the next shipped batch.
///
/// Pins the engine half of the wasm dpr bug: the browser embedder's
/// `ResizeObserver` alone never sees dpr-only changes (no CSS-box
/// change), which used to leave the canvas backing store at the stale
/// pixel ratio — the whole frame rendered at the old scale and the
/// compositor stretched it (blurry text at dpr 1.25). The embedder now
/// watches dpr directly (a `matchMedia` re-arm in `tur-wasm`); this test
/// guarantees the rail it feeds actually re-syncs.
#[test]
fn dpr_only_resize_resyncs_renderer() {
    let driver = TestSchedulerDriver::new();
    let pool = WorkerPoolHandle::new("dpr-only-resize", usize::MAX);
    let runtime = TurRuntime::builder()
        .worker_spawner(driver.worker_spawner())
        .host_loop(driver.host_loop())
        .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
        .clock(std::sync::Arc::new(MutexFixedClock::new(0)))
        .worker_pool(pool.clone())
        .plugin(TurStdPlugin)
        .build()
        .expect("runtime build");

    let (app, mut looper) = runtime
        .app_builder()
        .worker_pool(pool)
        .shell(TestShell::new(driver.vsync_source()))
        .build_headless((64.0, 64.0))
        .expect("headless build");
    let app = Rc::new(app);
    let (frame_tx, frame_rx) = mpsc::unbounded::<FrameOutcome>();
    looper.set_after_frame_hook(Some(Rc::new(move |o| {
        let _ = frame_tx.unbounded_send(o);
    })));
    driver.spawn_local(Box::pin(looper.run()));
    let frame_rx = RefCell::new(frame_rx);

    // Content that paints (an empty batch ships nothing — see
    // `deferred_resize` for the empty-paint caveat).
    block_on(app.load_rut_module(SOURCE)).expect("module load");
    pump_quiet(&driver, &frame_rx);

    // Attach AT the bootstrap size so the only later geometry change is
    // the dpr itself. Note the attach itself records no `resize`: the
    // builder seeded `last_viewport` with the build triple, so the
    // commit-point sync correctly dedups (in a real embedder the renderer
    // arrives already sized — it was CONSTRUCTED for that viewport).
    let calls = Rc::new(RefCell::new(Vec::new()));
    app.attach_renderer(
        Box::new(RecordingRenderer {
            calls: calls.clone(),
        }),
        64,
        64,
        1.0,
    );
    pump_quiet(&driver, &frame_rx);

    // dpr-only change: same logical size, doubled dpr.
    app.resize(64, 64, 2.0);
    pump_quiet(&driver, &frame_rx);
    assert!(
        calls.borrow().iter().any(|c| c == "resize:64x64@2"),
        "a dpr-only resize must re-sync the renderer (got {:?})",
        calls.borrow()
    );

    app.destroy();
}
