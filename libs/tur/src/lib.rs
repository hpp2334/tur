pub mod error;
pub use tur_boajs;
pub use tur_layout;
pub use tur_noop_renderer;
pub use tur_render_tree;
pub use tur_vello_renderer;
pub use tur_widget;

use boa_engine::Context;
use boa_engine::Source;
use error::TurError;
use tur_boajs::widget_tree;
use tur_layout::LayoutTree;
use tur_render_tree::{RenderTree, Renderer};
use tur_shared::Constraints;
use tur_widget::WidgetTree;

pub struct TurApp<R: Renderer> {
    context: Context,
    renderer: R,
}

impl<R: Renderer> TurApp<R> {
    pub fn new(renderer: R) -> Result<Self, TurError> {
        let mut context = Context::default();
        tur_boajs::init_bridge(&mut context);

        tracing::info!("TurApp initialized");

        Ok(TurApp { context, renderer })
    }

    pub fn load_js(&mut self, source: &str) -> Result<(), TurError> {
        self.context
            .eval(Source::from_bytes(source))
            .map_err(TurError::JsEval)?;
        tracing::info!("JS source loaded and executed ({} bytes)", source.len());
        Ok(())
    }

    pub fn widget_tree(&self) -> &'static LazyLock<RwLock<WidgetTree>> {
        widget_tree()
    }

    pub fn render(&mut self, width: f64, height: f64) {
        let constraints = Constraints {
            min_width: 0.0,
            max_width: width,
            min_height: 0.0,
            max_height: height,
        };

        let tree = widget_tree();
        let tree_guard = tree.read().unwrap();

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
}

use std::sync::{LazyLock, RwLock};
