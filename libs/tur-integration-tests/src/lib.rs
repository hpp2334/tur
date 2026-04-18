use std::path::Path;

use tur::error::TurError;
use tur::TurApp;
use tur_noop_renderer::NoopRenderer;
use tur_render_tree::RenderTree;
use tur_widget::WidgetTree;

pub struct TurTestApp {
    inner: TurApp<NoopRenderer>,
}

impl TurTestApp {
    pub fn new(width: f64, height: f64) -> Result<Self, TurError> {
        let mut inner = TurApp::new(NoopRenderer::new())?;
        inner.set_size(width, height);
        Ok(Self { inner })
    }

    pub fn load_bundle(&mut self, name: &str) -> Result<(), TurError> {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let workspace_root = Path::new(&manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .expect("failed to resolve workspace root");
        let path = workspace_root
            .join("js/packages/tur-test-cases/dist")
            .join(format!("{name}.js"));
        let source = std::fs::read_to_string(&path).map_err(TurError::Io)?;
        self.inner.load_js(&source)
    }

    pub fn load_bundle_raw(&mut self, source: &str) -> Result<(), TurError> {
        self.inner.load_js(source)
    }

    pub fn widget_tree(&self) -> WidgetTree {
        self.inner.widget_tree()
    }

    pub fn render_tree(&self) -> RenderTree {
        self.inner.render_tree()
    }
}
