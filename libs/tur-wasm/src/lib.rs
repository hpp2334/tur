#[cfg(target_arch = "wasm32")]
mod app;
pub mod fonts;

#[cfg(target_arch = "wasm32")]
pub use app::{AfterFrameHook, WasmAppConfig, WasmAppHandle};
pub use fonts::WasmFontLoader;

/// One-time wasm runtime init: install the panic hook (readable backtraces in
/// the browser console) and wire `tracing` events to `console.*`. The embedder
/// cdylib's `#[wasm_bindgen(start)]` entry calls this once.
pub fn init() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    tracing::info!("tur-wasm initialized");
}
