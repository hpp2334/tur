pub mod error;
pub use tur_boajs;
pub use tur_layout;
pub use tur_noop_renderer;
pub use tur_render_tree;
pub use tur_vello_renderer;
pub use tur_widget;

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::js_string;
use boa_engine::property::Attribute;
use boa_engine::Context;
use boa_engine::Source;
use error::TurError;
use tur_boajs::{BoaOpaque, TurAppContext};
use tur_layout::LayoutTree;
use tur_render_tree::{RenderTree, Renderer};
use tur_shared::Constraints;
use tur_widget::WidgetTree;

pub struct TurApp<R: Renderer> {
    context: Context,
    renderer: R,
    app_context: BoaOpaque<TurAppContext>,
    tree: Rc<RefCell<WidgetTree>>,
    size: (f64, f64),
}

impl<R: Renderer> TurApp<R> {
    pub fn new(renderer: R) -> Result<Self, TurError> {
        let mut context = Context::default();
        let (app_context, tree) = tur_boajs::init_bridge(&mut context);

        context
            .register_global_property(
                js_string!("__tur_ctx"),
                Into::<boa_engine::JsValue>::into(app_context.object().clone()),
                Attribute::all(),
            )
            .expect("failed to register __tur_ctx");

        tracing::info!("TurApp initialized");

        Ok(TurApp {
            context,
            renderer,
            app_context,
            tree,
            size: (400.0, 600.0),
        })
    }

    pub fn load_js(&mut self, source: &str) -> Result<(), TurError> {
        self.context
            .eval(Source::from_bytes(source))
            .map_err(TurError::JsEval)?;
        Ok(())
    }

    pub fn app_context(&self) -> &BoaOpaque<TurAppContext> {
        &self.app_context
    }

    pub fn set_size(&mut self, width: f64, height: f64) {
        self.size = (width, height);
    }

    pub fn render(&mut self) {
        let (width, height) = self.size;
        let constraints = Constraints {
            min_width: 0.0,
            max_width: width,
            min_height: 0.0,
            max_height: height,
        };

        let ctx = self
            .app_context
            .get()
            .expect("failed to downcast TurAppContext");
        let tree_guard = ctx.tree().borrow();

        let mut layout_tree = LayoutTree::from_widget_tree(&tree_guard);
        let result = layout_tree.compute_layout(&constraints);
        tracing::debug!("layout: {:?}", result.size);

        let render_tree = RenderTree::from_layout_tree(&layout_tree, &tree_guard);
        self.renderer.render(&render_tree);
        tracing::debug!("rendered: {:?}", result.size);
    }

    pub fn renderer_mut(&mut self) -> &mut R {
        &mut self.renderer
    }

    #[cfg(feature = "trace")]
    pub fn widget_tree(&self) -> std::cell::Ref<'_, WidgetTree> {
        self.tree.borrow()
    }

    #[cfg(feature = "trace")]
    pub fn render_tree(&self) -> RenderTree {
        let (width, height) = self.size;
        let constraints = Constraints {
            min_width: 0.0,
            max_width: width,
            min_height: 0.0,
            max_height: height,
        };
        let tree_guard = self.tree.borrow();
        let mut layout_tree = LayoutTree::from_widget_tree(&tree_guard);
        layout_tree.compute_layout(&constraints);
        RenderTree::from_layout_tree(&layout_tree, &tree_guard)
    }
}
