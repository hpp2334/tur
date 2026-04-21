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
#[cfg(feature = "trace")]
use core::elements::ElementTree;
use core::event::{EventKind, RawAppEvent};

pub struct TurApp {
    boa_context: Context,
    app_context: Rc<RefCell<TurAppContext>>,
}

impl TurApp {
    pub fn new(renderer: Box<dyn core::render::Renderer>) -> Result<Self, TurError> {
        let mut boa_context = Context::default();
        let app_context = init_bridge(&mut boa_context, renderer);

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

    #[cfg(feature = "trace")]
    pub fn element_tree(&self) -> Rc<RefCell<ElementTree>> {
        self.app_context.borrow().element_tree_rc()
    }
}
