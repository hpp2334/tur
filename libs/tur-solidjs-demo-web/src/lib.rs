mod app;
mod render;

pub use app::TurDemoApp;

use tracing;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn wasm_entry() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    tracing::info!("tur-solidjs-demo-web starting");

    let mut app = match TurDemoApp::new() {
        Ok(app) => app,
        Err(e) => {
            tracing::error!("Failed to create TurDemoApp: {e}");
            return;
        }
    };

    if let Err(e) = app.load_and_run() {
        tracing::error!("Failed to run demo: {e}");
    }
}
