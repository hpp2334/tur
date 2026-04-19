pub mod error;
pub use tur_boajs;
pub use tur_element_tree;
pub use tur_noop_renderer;
pub use tur_render_tree;
pub use tur_vello_renderer;

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::Context;
use boa_engine::Source;
use error::TurError;
use tur_boajs::TurAppContext;
#[cfg(feature = "trace")]
use tur_element_tree::ElementTree;
use tur_render_tree::{RenderTree, Renderer};
pub struct TurApp {
    boa_context: Context,
    app_context: Rc<RefCell<TurAppContext>>,
}

impl TurApp {
    pub fn new(renderer: Box<dyn Renderer>) -> Result<Self, TurError> {
        let mut boa_context = Context::default();
        let app_context = tur_boajs::init_bridge(&mut boa_context, renderer);

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

    pub fn render(&mut self) {
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

    #[cfg(feature = "trace")]
    pub fn element_tree(&self) -> Rc<RefCell<ElementTree>> {
        self.app_context.borrow().element_tree_rc()
    }

    #[cfg(feature = "trace")]
    pub fn render_tree(&self) -> Rc<RefCell<RenderTree>> {
        let ctx = self.app_context.borrow();
        ctx.render();
        ctx.render_tree_rc()
    }
}
