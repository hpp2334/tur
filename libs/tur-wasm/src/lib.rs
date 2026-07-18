#[cfg(target_arch = "wasm32")]
mod app;
pub mod fonts;

use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
pub use app::TurWasmApp;
pub use fonts::WasmFontLoader;

#[wasm_bindgen(start)]
pub fn wasm_entry() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    tracing::info!("tur-wasm initialized");
}
