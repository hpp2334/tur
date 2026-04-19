use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use tur::error::TurError;
use tur::TurApp;
use tur_element_tree::ElementTree;
use tur_noop_renderer::NoopRenderer;
use tur_render_tree::RenderTree;

pub struct TurTestApp {
    inner: TurApp,
}

impl TurTestApp {
    pub fn new(width: f64, height: f64) -> Result<Self, TurError> {
        let mut inner = TurApp::new(Box::new(NoopRenderer::new()))?;
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

    pub fn element_tree(&self) -> ElementTree {
        self.inner.element_tree()
    }

    pub fn render_tree(&self) -> Rc<RefCell<RenderTree>> {
        self.inner.render_tree()
    }
}
