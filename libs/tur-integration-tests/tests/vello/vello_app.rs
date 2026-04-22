use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use minifb::{Window, WindowOptions};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tur_engine::core::elements::ElementTree;
use tur_engine::error::TurError;
use tur_engine::renderer::vello::VelloRenderer;
use tur_engine::TurApp;

pub struct TurVelloApp {
    inner: RefCell<TurVelloAppInner>,
}

struct TurVelloAppInner {
    app: TurApp,
    _window: Window,
}

impl TurVelloApp {
    pub fn new(width: f64, height: f64, dpr: f64) -> Result<Self, TurError> {
        pollster::block_on(Self::init_async(width, height, dpr))
    }

    async fn init_async(width: f64, height: f64, dpr: f64) -> Result<Self, TurError> {
        let window = Window::new(
            "tur-vello-test",
            width as usize,
            height as usize,
            WindowOptions {
                resize: false,
                ..Default::default()
            },
        )
        .map_err(|e| TurError::Other(e.to_string()))?;

        let instance = vello::wgpu::Instance::new(vello::wgpu::InstanceDescriptor {
            backends: vello::wgpu::Backends::all(),
            ..Default::default()
        });

        let raw_display = window.display_handle().map_err(|e| {
            TurError::Other(format!("failed to get display handle: {e}"))
        })?;
        let raw_window = window.window_handle().map_err(|e| {
            TurError::Other(format!("failed to get window handle: {e}"))
        })?;

        let surface = unsafe {
            instance
                .create_surface_unsafe(vello::wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: raw_display.as_raw(),
                    raw_window_handle: raw_window.as_raw(),
                })
                .map_err(|e| TurError::Other(e.to_string()))?
        };

        let adapter = instance
            .request_adapter(&vello::wgpu::RequestAdapterOptions {
                power_preference: vello::wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| TurError::Other("failed to request adapter".into()))?;

        let (device, queue) = adapter
            .request_device(
                &vello::wgpu::DeviceDescriptor {
                    label: None,
                    required_features: vello::wgpu::Features::empty(),
                    required_limits: vello::wgpu::Limits::default(),
                    memory_hints: vello::wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| TurError::Other(e.to_string()))?;

        let renderer = VelloRenderer::init_surface(
            &adapter,
            device,
            queue,
            surface,
            width as u32,
            height as u32,
            dpr,
        );

        let mut app = TurApp::new(Box::new(renderer))?;
        app.set_size(width, height);

        Ok(TurVelloApp {
            inner: RefCell::new(TurVelloAppInner {
                app,
                _window: window,
            }),
        })
    }

    pub fn load_bundle(&self, name: &str) -> Result<(), TurError> {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let workspace_root = Path::new(&manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .expect("failed to resolve workspace root");
        let path = workspace_root
            .join("js/packages/tur-test-cases/dist")
            .join(format!("{name}.js"));
        let source = std::fs::read_to_string(&path).map_err(TurError::Io)?;
        self.inner.borrow_mut().app.load_js(&source)
    }

    pub fn load_bundle_raw(&self, source: &str) -> Result<(), TurError> {
        self.inner.borrow_mut().app.load_js(source)
    }

    pub fn element_tree(&self) -> Rc<RefCell<ElementTree>> {
        self.inner.borrow().app.element_tree()
    }

    pub fn render(&self) {
        self.inner.borrow().app.render();
    }

    pub fn present(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.inner.borrow().app.present()
    }

    pub fn renderer_resize(&self, logical_width: u32, logical_height: u32, dpr: f64) {
        self.inner
            .borrow()
            .app
            .renderer_resize(logical_width, logical_height, dpr);
    }

    pub fn set_size(&self, width: f64, height: f64) {
        self.inner.borrow_mut().app.set_size(width, height);
    }
}
