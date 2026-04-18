mod app;

use wasm_bindgen::prelude::*;

pub use app::TurWasmApp;

#[wasm_bindgen(start)]
pub fn wasm_entry() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    tracing::info!("tur-wasm initialized");
}
