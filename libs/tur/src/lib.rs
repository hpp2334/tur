pub mod error;
pub use tur_boajs;
pub use tur_layout;
pub use tur_noop_renderer;
pub use tur_render_tree;
pub use tur_vello_renderer;
pub use tur_widget;

use boa_engine::js_string;
use boa_engine::property::Attribute;
use boa_engine::Context;
use boa_engine::Source;
use error::TurError;
use tur_boajs::{BoaOpaque, TurAppContext};
use tur_render_tree::{RenderTree, Renderer};

pub struct TurApp {
    boa_context: Context,
    app_context: BoaOpaque<TurAppContext>,
}

impl TurApp {
    pub fn new(renderer: Box<dyn Renderer>) -> Result<Self, TurError> {
        let mut boa_context = Context::default();
        let app_context = tur_boajs::init_bridge(&mut boa_context, renderer);

        boa_context
            .register_global_property(
                js_string!("__tur_ctx"),
                Into::<boa_engine::JsValue>::into(app_context.object().clone()),
                Attribute::all(),
            )
            .expect("failed to register __tur_ctx");

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

    pub fn app_context(&self) -> &BoaOpaque<TurAppContext> {
        &self.app_context
    }

    pub fn set_size(&mut self, width: f64, height: f64) {
        let ctx = self
            .app_context
            .get()
            .expect("failed to downcast TurAppContext");
        ctx.set_size(width, height);
    }

    pub fn render(&mut self) {
        let ctx = self
            .app_context
            .get()
            .expect("failed to downcast TurAppContext");
        ctx.render();
    }

    pub fn present(&self) -> Result<(), Box<dyn std::error::Error>> {
        let ctx = self
            .app_context
            .get()
            .expect("failed to downcast TurAppContext");
        ctx.renderer().borrow_mut().present()
    }

    pub fn renderer_resize(&self, logical_width: u32, logical_height: u32, dpr: f64) {
        let ctx = self
            .app_context
            .get()
            .expect("failed to downcast TurAppContext");
        ctx.renderer()
            .borrow_mut()
            .resize(logical_width, logical_height, dpr);
    }

    #[cfg(feature = "trace")]
    pub fn widget_tree(&self) -> tur_widget::WidgetTree {
        self.app_context
            .get()
            .expect("failed to downcast TurAppContext")
            .tree()
            .borrow()
            .clone()
    }

    #[cfg(feature = "trace")]
    pub fn render_tree(&self) -> RenderTree {
        let ctx = self
            .app_context
            .get()
            .expect("failed to downcast TurAppContext");
        ctx.render();
        ctx.render_tree().borrow().clone()
    }
}
