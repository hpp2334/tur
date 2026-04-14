use tur::TurApp;
use tur_vello_renderer::VelloRenderer;

#[cfg(feature = "embedded_js")]
const JS_BUNDLE: &str = include_str!("../../../../js/packages/tur-solidjs-demo/dist/bundle.js");

pub struct TurDemoApp {
    #[allow(dead_code)]
    app: TurApp<VelloRenderer>,
}

impl TurDemoApp {
    pub fn new() -> anyhow::Result<Self> {
        let renderer = VelloRenderer::new()?;
        let app = TurApp::new(renderer)?;
        Ok(TurDemoApp { app })
    }

    pub fn load_and_run(&mut self) -> anyhow::Result<()> {
        #[cfg(feature = "embedded_js")]
        {
            self.app.load_js(JS_BUNDLE)?;
            self.app.call_start_app()?;
            tracing::info!("JS demo loaded and startApp() executed");
        }

        #[cfg(not(feature = "embedded_js"))]
        {
            tracing::warn!("embedded_js feature not enabled, skipping JS load");
        }

        Ok(())
    }

    pub fn run_event_loop(self) {
        tracing::info!("event loop would run here (requires winit web backend)");
        todo!("wire up winit event loop for web canvas rendering");
    }
}
