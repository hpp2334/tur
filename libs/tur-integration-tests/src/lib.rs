use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use tur_engine::core::element::ElementNodeId;
use tur_engine::core::elements::ElementTree;
use tur_engine::error::TurError;
use tur_engine::renderer::noop::NoopRenderer;
use tur_engine::TurApp;

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

    pub fn element_tree(&self) -> Rc<RefCell<ElementTree>> {
        self.inner.element_tree()
    }

    pub fn render_tree(&self) -> Rc<RefCell<ElementTree>> {
        self.inner.render();
        self.inner.element_tree()
    }

    pub fn handle_pointer_down(&self, x: f64, y: f64) {
        self.inner.handle_pointer_down(x, y);
    }

    pub fn handle_pointer_up(&mut self, x: f64, y: f64) {
        self.inner.handle_pointer_up(x, y);
    }

    pub fn click(&mut self, x: f64, y: f64) {
        self.handle_pointer_down(x, y);
        self.handle_pointer_up(x, y);
    }

    pub fn has_event_handler(&self, id: ElementNodeId, event_type: &str) -> bool {
        self.inner.has_event_handler(id, event_type)
    }

    pub fn text_content(&self, id: ElementNodeId) -> Option<String> {
        let tree = self.inner.element_tree();
        let tree = tree.borrow();
        tree.get(id)
            .and_then(|n| n.element.as_ref())
            .and_then(|e| e.text_content())
            .map(|s| s.to_string())
    }
}
