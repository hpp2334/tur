use tracing;
use tur::TurApp;

#[cfg(feature = "embedded_js")]
const JS_BUNDLE: &str = include_str!("../../../../js/packages/tur-solidjs-demo/dist/bundle.js");

pub struct TurDemoApp {
    app: TurApp,
}

impl TurDemoApp {
    pub fn new() -> anyhow::Result<Self> {
        let app = TurApp::new()?;
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
