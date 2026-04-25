pub mod core;
pub mod elements;
pub mod renderer;

pub mod error;

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::Context;
use boa_engine::Source;
use error::TurError;

use core::bridge::init_bridge;
use core::bridge::TurAppContext;
use core::element::ElementNodeId;
use core::elements::AnyElement;
#[cfg(feature = "trace")]
use core::elements::ElementTree;
use core::event::{EventKind, RawAppEvent};
pub use core::fonts::{FontLoader, FontManager, PresetFontLoader};

pub struct TurApp {
    boa_context: Context,
    app_context: Rc<RefCell<TurAppContext>>,
}

impl TurApp {
    pub fn new(
        renderer: Box<dyn core::render::Renderer>,
        font_manager: FontManager,
    ) -> Result<Self, TurError> {
        let mut boa_context = Context::default();
        let app_context = init_bridge(&mut boa_context, renderer, font_manager);

        tracing::info!("TurApp initialized");

        Ok(TurApp {
            boa_context,
            app_context,
        })
    }

    pub fn load_js(&mut self, source: &str) -> Result<(), TurError> {
        self.boa_context
            .eval(Source::from_bytes(source))
            .map_err(TurError::JsEval)?;
        Ok(())
    }

    pub fn app_context(&self) -> &Rc<RefCell<TurAppContext>> {
        &self.app_context
    }

    pub fn set_size(&mut self, width: f64, height: f64) {
        self.app_context.borrow().set_size(width, height);
    }

    pub fn render(&self) {
        self.app_context.borrow().render();
    }

    pub fn debug_layout(&self) -> String {
        self.app_context
            .borrow()
            .element_tree()
            .borrow()
            .debug_layout()
    }

    pub fn present(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.app_context.borrow().renderer().borrow_mut().present()
    }

    pub fn renderer_resize(&self, logical_width: u32, logical_height: u32, dpr: f64) {
        self.app_context.borrow().renderer().borrow_mut().resize(
            logical_width,
            logical_height,
            dpr,
        );
    }

    pub fn dispatch_raw_event(&mut self, event: RawAppEvent) {
        self.app_context
            .borrow()
            .dispatch_raw_event(&event, &mut self.boa_context);
    }

    pub fn has_event_handler(&self, id: ElementNodeId, kind: EventKind) -> bool {
        self.app_context.borrow().has_event_handler(id, kind)
    }

    pub fn query_element(&self, key: &[&str]) -> Option<ElementNodeId> {
        self.app_context
            .borrow()
            .element_tree()
            .borrow()
            .query_element(key)
    }

    pub fn with_element<R>(
        &self,
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R,
    ) -> Option<R> {
        let ctx = self.app_context.borrow();
        let tree = ctx.element_tree().borrow();
        let node = tree.get(id)?;
        let element = node.element.as_ref()?;
        Some(cb(element))
    }

    #[cfg(feature = "trace")]
    pub fn element_tree(&self) -> Rc<RefCell<ElementTree>> {
        self.app_context.borrow().element_tree_rc()
    }
}
