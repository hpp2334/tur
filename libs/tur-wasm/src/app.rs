use tur::TurApp;
use tur_vello_renderer::VelloRenderer;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct TurWasmApp {
    app: TurApp<VelloRenderer>,
}

#[wasm_bindgen]
impl TurWasmApp {
    pub fn create() -> Result<TurWasmApp, JsValue> {
        let renderer = VelloRenderer::new().map_err(|e| JsValue::from_str(&e.to_string()))?;
        let app = TurApp::new(renderer).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(TurWasmApp { app })
    }

    pub fn load_and_run_js(&mut self, js_source: &str) -> Result<(), JsValue> {
        self.app
            .load_js(js_source)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.app
            .call_start_app()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        tracing::info!("JS loaded and startApp() executed");
        Ok(())
    }
}
