use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use boa_engine::context::time::StdClock;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use minifb::{Window, WindowOptions};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tur_engine::TurStdPlugin;
use tur_engine::core::app::{FrameOutcome, NextFrame};
use tur_engine::core::scheduler::MainSchedulerDriver;
use tur_engine::error::TurError;
use tur_engine::renderer::vello::{RawSurface, WgpuRenderer};
use tur_engine::{TurApp, TurRuntime};
use tur_native::NativeFontLoader;

#[derive(Debug, thiserror::Error)]
pub enum TurVelloError {
    #[error(transparent)]
    Engine(#[from] TurError),
    #[error("window creation failed: {0}")]
    Window(String),
    #[error("raw window handle error: {0}")]
    Handle(String),
}

/// Test harness that drives a real `VelloRenderer` on the main thread via
/// the production `TurApp::run_loop`.
///
/// The harness installs an `after_frame` hook feeding a frame channel,
/// spawns `run_loop` once, and exposes `wait_for` / `wait_for_timeout`.
/// `run_loop`'s pipelining is safe for pixel readback because each
/// `RenderCommands` batch is a full scene rebuild (`scene.reset()`), and
/// the loop flushes the latest batch at quiescence *before* the
/// `after_frame` hook fires — so `render_to_pixels` taken after
/// `wait_for_timeout(ZERO)` reads the fully-settled frame.
pub struct TurVelloApp {
    inner: RefCell<TurVelloAppInner>,
}

struct TurVelloAppInner {
    app: Rc<TurApp>,
    driver: Rc<tur_integration_tests::TestSchedulerDriver>,
    frame_rx: futures::channel::mpsc::UnboundedReceiver<FrameOutcome>,
    _window: Window,
}

/// Explicit shutdown: `destroy()` tells the worker to exit (breaking the
/// run_loop's channel waits) so the final `settle_local_tasks` in `main`
/// completes the run_loop and drops the app — render targets and all —
/// while the thread's TLS is still alive. Without it the run_loop sits in
/// the thread-local `LocalSet` until thread destruction, where dropping
/// wgpu buffers after TLS teardown panics in the destructor (order
/// roulette between the LocalSet TLS and wgpu's internal TLS).
impl Drop for TurVelloAppInner {
    fn drop(&mut self) {
        self.app.destroy();
    }
}

impl TurVelloApp {
    pub fn new(width: f64, height: f64, dpr: f64) -> Result<Self, TurVelloError> {
        let (app, driver, window) = pollster::block_on(Self::init_async(width, height, dpr))?;

        // Spawn the autonomous `run_loop`. The `after_frame` hook ships each
        // `FrameOutcome` into `frame_rx`; `drive_one_frame` pairs one
        // `fire_vsync` with one awaited outcome.
        let (frame_tx, frame_rx) = futures::channel::mpsc::unbounded::<FrameOutcome>();
        app.set_after_frame_hook(Some(Rc::new(move |o| {
            let _ = frame_tx.unbounded_send(o);
        })));
        driver.spawn_local(Box::pin(app.clone().run_loop()));

        let harness = TurVelloApp {
            inner: RefCell::new(TurVelloAppInner {
                app,
                driver,
                frame_rx,
                _window: window,
            }),
        };
        // Bootstrap: drive the initial self-paint frame.
        let _ = harness.drive_one_frame();
        Ok(harness)
    }

    async fn init_async(
        width: f64,
        height: f64,
        dpr: f64,
    ) -> Result<
        (
            Rc<TurApp>,
            Rc<tur_integration_tests::TestSchedulerDriver>,
            Window,
        ),
        TurVelloError,
    > {
        let window = Window::new(
            "tur-vello-test",
            width as usize,
            height as usize,
            WindowOptions {
                resize: false,
                ..Default::default()
            },
        )
        .map_err(|e| TurVelloError::Window(e.to_string()))?;

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });

        let raw_display = window
            .display_handle()
            .map_err(|e| TurVelloError::Handle(format!("display: {e}")))?;
        let raw_window = window
            .window_handle()
            .map_err(|e| TurVelloError::Handle(format!("window: {e}")))?;

        let driver = tur_integration_tests::TestSchedulerDriver::new();
        let runtime = TurRuntime::builder()
            .scheduler(driver.clone())
            .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
            .clock(std::sync::Arc::new(StdClock::new()))
            .plugin(TurStdPlugin)
            .plugin(tur_animation::TurAnimationPlugin)
            .build()?;

        // Threaded engine: worker produces command batches; `MainBackend`
        // owns the per-root VelloTarget on main and applies them via
        // `run_loop`. The root is declared pending at build; the surface
        // (raw window handles off the shared instance) attaches right here
        // via `setup_root`.
        let app = runtime
            .app_builder()
            .renderer(Box::new(WgpuRenderer::with_instance(instance)))
            .view_root("main", (width, height), dpr)
            .build()?;
        app.setup_root(
            "main",
            Box::new(RawSurface {
                raw_display_handle: raw_display.as_raw(),
                raw_window_handle: raw_window.as_raw(),
            }),
            (width, height),
            dpr,
        )?;
        Ok((app, driver, window))
    }

    /// Drive one frame: drain stale self-wake outcomes, kick the vsync, and
    /// block (driving the LocalSet) until the next frame's `after_frame`.
    fn drive_one_frame(&self) -> FrameOutcome {
        // Clone the driver Rc out first so `block_on` (immutable driver
        // borrow) doesn't conflict with the mutable `frame_rx` borrow below.
        let driver = self.inner.borrow().driver.clone();
        let mut inner = self.inner.borrow_mut();
        while let Some(Some(_stale)) = inner.frame_rx.next().now_or_never() {}
        driver.fire_vsync();
        driver
            .block_on(inner.frame_rx.next())
            .expect("worker destroyed mid-frame")
    }

    /// Drive `frames`-worth of frames, each to quiescence. `ZERO` drives a
    /// single frame to quiescence — sufficient for pixel readback because
    /// `run_loop` flushes the latest batch before the `after_frame` hook
    /// fires.
    pub fn wait_for_timeout(&self, timeout: Duration) {
        let frames = (timeout.as_millis() as u64).div_ceil(16);
        let iters = frames.max(1);
        for _ in 0..iters {
            // Drive to quiescence at this tick (cap 8 sub-iterations).
            for _ in 0..8 {
                let outcome = self.drive_one_frame();
                if !outcome.rendered && outcome.schedule == NextFrame::Idle {
                    break;
                }
            }
        }
    }

    pub fn load_bundle(&self, name: &str) -> Result<(), TurVelloError> {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let workspace_root = Path::new(&manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .expect("failed to resolve workspace root");
        let path = workspace_root
            .join("js/packages/tur-test-cases/dist")
            .join(format!("{name}.js"));
        let source = std::fs::read_to_string(&path).map_err(TurError::Io)?;
        futures::executor::block_on(self.inner.borrow().app.load_module(&source))?;
        // Drive the module's initial render to quiescence.
        self.wait_for_timeout(Duration::ZERO);
        Ok(())
    }

    /// Direct access to the underlying `TurApp`.
    pub fn app(&self) -> std::cell::Ref<'_, Rc<TurApp>> {
        std::cell::Ref::map(self.inner.borrow(), |i| &i.app)
    }

    /// Read rendered pixels back from the app-owned renderer. Call after
    /// `wait_for_timeout(ZERO)` so the latest batch is flushed.
    pub fn render_to_pixels(&self) -> Vec<u8> {
        self.inner
            .borrow()
            .app
            .render_to_pixels()
            .expect("renderer does not support render_to_pixels")
    }
}
