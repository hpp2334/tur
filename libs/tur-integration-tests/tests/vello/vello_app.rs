use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use boa_engine::context::time::StdClock;
use minifb::{Window, WindowOptions};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tur_engine::core::elements::NodeTreeData;
use tur_engine::core::event::PlatformEvent;
use tur_native::NativeFontLoader;
use tur_engine::error::TurError;
use tur_engine::renderer::vello::VelloRenderer;
use tur_engine::{TurApp, TurEngine};
use tur_std::TurStdPlugin;

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

        let app = TurEngine::builder()
            .renderer(Box::new(renderer))
            .font_loader(Box::new(NativeFontLoader::new()))
            .clock(Rc::new(StdClock::new()))
            .plugin(TurStdPlugin)
            .plugin(tur_animation::TurAnimationPlugin)
            .build()?;
        app.push_platform_event(PlatformEvent::Resize {
            logical_width: width as u32,
            logical_height: height as u32,
            dpr,
        });
        let _ = app.run_frame();

        Ok(TurVelloApp {
            inner: RefCell::new(TurVelloAppInner {
                app,
                _window: window,
            }),
        })
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
        self.inner.borrow().app.load_module(&source)?;
        Ok(())
    }

    pub fn with_element_tree<R>(&self, f: impl FnOnce(&NodeTreeData) -> R) -> R {
        let inner = self.inner.borrow();
        let tree = inner.app.element_tree();
        f(&tree)
    }

    pub fn render(&self) {
        self.inner.borrow().app.request_paint();
        let _ = self.inner.borrow().app.run_frame();
    }

    pub fn render_to_pixels(&self) -> Vec<u8> {
        self.inner.borrow().app.render_to_pixels().expect("renderer does not support render_to_pixels")
    }
}
