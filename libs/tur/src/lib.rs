pub use tur_boajs;
pub use tur_vello_renderer;
pub use tur_widget;

use boa_engine::Context;
use boa_engine::Source;
use tracing;
use tur_boajs::widget_tree;
use tur_vello_renderer::VelloRenderer;
use tur_widget::{Constraints, WidgetTree};

pub struct TurApp {
    context: Context,
    renderer: VelloRenderer,
}

impl TurApp {
    pub fn new() -> anyhow::Result<Self> {
        let mut context = Context::default();
        tur_boajs::init_bridge(&mut context);
        let renderer = VelloRenderer::new()?;

        tracing::info!("TurApp initialized");

        Ok(TurApp { context, renderer })
    }

    pub fn load_js(&mut self, source: &str) -> anyhow::Result<()> {
        self.context
            .eval(Source::from_bytes(source))
            .map_err(|e| anyhow::anyhow!("JS evaluation failed: {e}"))?;
        tracing::info!("JS source loaded ({} bytes)", source.len());
        Ok(())
    }

    pub fn call_start_app(&mut self) -> anyhow::Result<()> {
        self.context
            .eval(Source::from_bytes("globalThis.startApp()"))
            .map_err(|e| anyhow::anyhow!("startApp() failed: {e}"))?;
        tracing::info!("startApp() called");
        Ok(())
    }

    pub fn widget_tree(&self) -> &'static LazyLock<RwLock<WidgetTree>> {
        widget_tree()
    }

    pub fn render(&mut self, width: f64, height: f64) -> anyhow::Result<()> {
        let constraints = Constraints {
            min_width: 0.0,
            max_width: width,
            min_height: 0.0,
            max_height: height,
        };
        let mut tree = widget_tree().write().unwrap();
        let result = self.renderer.render_to_scene(&mut tree, &constraints);
        tracing::debug!("rendered: {:?}", result.size);
        Ok(())
    }

    pub fn renderer_mut(&mut self) -> &mut VelloRenderer {
        &mut self.renderer
    }
}

use std::sync::{LazyLock, RwLock};
