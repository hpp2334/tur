#[cfg(target_arch = "wasm32")]
mod app;
mod compiler;

use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
pub use app::TurWasmApp;

pub use compiler::{transpile_tsx, tokenize_tsx, TokenSpan};

#[wasm_bindgen(start)]
pub fn wasm_entry() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    tracing::info!("tur-wasm initialized");
}
