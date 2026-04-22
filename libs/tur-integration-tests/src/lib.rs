use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use tur_engine::core::element::ElementNodeId;
use tur_engine::core::elements::AnyElement;
use tur_engine::core::elements::ElementTree;
use tur_engine::core::event::{EventKind, RawAppEvent};
use tur_engine::error::TurError;
use tur_engine::renderer::noop::NoopRenderer;
use tur_engine::TurApp;
use tur_shared::Offset;

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

    pub fn render(&self) {
        self.inner.render();
    }

    pub fn element_tree(&self) -> Rc<RefCell<ElementTree>> {
        self.inner.element_tree()
    }

    pub fn dispatch_raw_event(&mut self, event: RawAppEvent) {
        self.inner.dispatch_raw_event(event);
    }

    pub fn click(&mut self, x: f64, y: f64) {
        self.dispatch_raw_event(RawAppEvent::PointerDown {
            position: Offset::new(x, y),
        });
        self.dispatch_raw_event(RawAppEvent::PointerUp {
            position: Offset::new(x, y),
        });
    }

    pub fn has_event_handler(&self, id: ElementNodeId, kind: EventKind) -> bool {
        self.inner.has_event_handler(id, kind)
    }

    pub fn query_element(&self, key: &[&str]) -> Option<ElementNodeId> {
        self.inner.query_element(key)
    }

    pub fn with_element<R>(
        &self,
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R,
    ) -> Option<R> {
        self.inner.with_element(id, cb)
    }
}
