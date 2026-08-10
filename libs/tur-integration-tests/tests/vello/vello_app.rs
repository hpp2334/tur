use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use boa_engine::context::time::StdClock;
use minifb::{Window, WindowOptions};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tur_engine::TurStdPlugin;
use tur_engine::error::TurError;
use tur_engine::renderer::vello::VelloRenderer;
use tur_engine::{TurApp, TurRuntime};
use tur_native::NativeFontLoader;

#[derive(Debug, thiserror::Error)]
pub enum TurVelloError {
    #[error(transparent)]
    Engine(#[from] TurError),
    #[error("window creation failed: {0}")]
    Window(String),
    #[error("wgpu adapter request failed: {0}")]
    WgpuAdapter(String),
    #[error("wgpu device request failed: {0}")]
    WgpuDevice(String),
    #[error("wgpu surface creation failed: {0}")]
    WgpuSurface(String),
    #[error("window handle error: {0}")]
    Handle(String),
}

/// Test harness that drives a real `VelloRenderer` on the main thread.
///
/// Uses `TurApp::pump` (immediate-render single-frame primitive) rather than
/// `run_loop`: pixel-readback tests need the worker's `RenderCommands` applied
/// synchronously within the frame they're produced, with no vsync pipelining
/// (whose latest-wins buffering can leave a transient partial batch as the
/// last render before a snapshot). Both `pump` and `run_loop` route through
/// the shared `apply_msg`, so this is not a divergent handler.
pub struct TurVelloApp {
    inner: RefCell<TurVelloAppInner>,
}

struct TurVelloAppInner {
    app: Rc<TurApp>,
    _window: Window,
}

impl TurVelloApp {
    pub fn new(width: f64, height: f64, dpr: f64) -> Result<Self, TurVelloError> {
        pollster::block_on(Self::init_async(width, height, dpr))
    }

    async fn init_async(width: f64, height: f64, dpr: f64) -> Result<Self, TurVelloError> {
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

        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(raw_display.as_raw()),
                    raw_window_handle: raw_window.as_raw(),
                })
                .map_err(|e| TurVelloError::WgpuSurface(e.to_string()))?
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| TurVelloError::WgpuAdapter(e.to_string()))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
            })
            .await
            .map_err(|e| TurVelloError::WgpuDevice(e.to_string()))?;

        let renderer = VelloRenderer::init_surface(
            &adapter,
            device,
            queue,
            surface,
            width as u32,
            height as u32,
            dpr,
        );

        let runtime = TurRuntime::builder()
            .scheduler(tur_integration_tests::TestSchedulerDriver::new())
            .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
            .clock(std::sync::Arc::new(StdClock::new()))
            .plugin(TurStdPlugin)
            .plugin(tur_animation::TurAnimationPlugin)
            .build()?;

        // Threaded engine: worker produces command batches; `MainBackend`
        // owns the VelloRenderer on main and applies them directly.
        let app = runtime.create_app(Box::new(renderer), (width, height), dpr)?;
        // Bootstrap: drive the initial self-paint frame.
        let _ = futures::executor::block_on(app.pump());
        Ok(TurVelloApp {
            inner: RefCell::new(TurVelloAppInner {
                app,
                _window: window,
            }),
        })
    }

    /// Drive one frame (immediate render).
    fn pump(&self) {
        let _ = futures::executor::block_on(self.inner.borrow().app.pump());
    }

    /// Drive `n` frames (immediate render each). The pixel-readback tests need
    /// a settled, complete render; pumping a handful of frames reaches
    /// quiescence without run_loop's pipelining.
    pub fn wait_for_timeout(&self, timeout: Duration) {
        let frames = ((timeout.as_millis() as u64) + 15) / 16;
        let iters = frames.max(1);
        for _ in 0..iters {
            self.pump();
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
        self.wait_for_timeout(Duration::from_millis(64));
        Ok(())
    }

    /// Direct access to the underlying `TurApp`.
    pub fn app(&self) -> std::cell::Ref<'_, Rc<TurApp>> {
        std::cell::Ref::map(self.inner.borrow(), |i| &i.app)
    }

    /// Read rendered pixels back from the app-owned renderer.
    pub fn render_to_pixels(&self) -> Vec<u8> {
        self.inner
            .borrow()
            .app
            .render_to_pixels()
            .expect("renderer does not support render_to_pixels")
    }
}
