use std::cell::RefCell;
use std::rc::Rc;
use tur::TurApp;
use tur_vello_renderer::VelloRenderer;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::future_to_promise;

struct WasmState {
    app: TurApp<VelloRenderer>,
    canvas: web_sys::HtmlCanvasElement,
    _resize_closure: Closure<dyn Fn()>,
}

#[wasm_bindgen]
pub struct TurWasmApp {
    state: Rc<RefCell<Option<WasmState>>>,
}

trait JsResult<T> {
    fn err_to_jsval(self) -> Result<T, JsValue>;
}

impl<T, E: Into<JsValue>> JsResult<T> for Result<T, E> {
    fn err_to_jsval(self) -> Result<T, JsValue> {
        self.map_err(Into::into)
    }
}

#[wasm_bindgen]
impl TurWasmApp {
    pub fn create() -> js_sys::Promise {
        let state: Rc<RefCell<Option<WasmState>>> = Rc::new(RefCell::new(None));
        let state_clone = state.clone();

        future_to_promise(async move {
            let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
            let document = window.document().ok_or_else(|| JsValue::from_str("no document"))?;

            let canvas = document
                .create_element("canvas")
                .err_to_jsval()?
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .err_to_jsval()?;

            let body = document.body().ok_or_else(|| JsValue::from_str("no body"))?;
            body.append_child(&canvas).err_to_jsval()?;

            canvas
                .style()
                .set_property("width", "100vw")
                .err_to_jsval()?;
            canvas
                .style()
                .set_property("height", "100vh")
                .err_to_jsval()?;
            canvas
                .style()
                .set_property("display", "block")
                .err_to_jsval()?;

            body.style()
                .set_property("margin", "0")
                .err_to_jsval()?;
            body.style()
                .set_property("overflow", "hidden")
                .err_to_jsval()?;

            let dpr = window.device_pixel_ratio();
            let logical_width = window
                .inner_width()
                .err_to_jsval()?
                .as_f64()
                .unwrap_or(800.0) as u32;
            let logical_height = window
                .inner_height()
                .err_to_jsval()?
                .as_f64()
                .unwrap_or(600.0) as u32;

            let physical_width = (logical_width as f64 * dpr) as u32;
            let physical_height = (logical_height as f64 * dpr) as u32;
            canvas.set_width(physical_width);
            canvas.set_height(physical_height);

            let instance = vello::wgpu::Instance::new(vello::wgpu::InstanceDescriptor {
                backends: vello::wgpu::Backends::GL,
                ..Default::default()
            });

            let surface = instance
                .create_surface(vello::wgpu::SurfaceTarget::Canvas(canvas.clone()))
                .map_err(|e| JsValue::from_str(&format!("failed to create surface: {e}")))?;

            let adapter = instance
                .request_adapter(&vello::wgpu::RequestAdapterOptions {
                    power_preference: vello::wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .ok_or_else(|| JsValue::from_str("failed to request adapter"))?;

            let (device, queue) = adapter
                .request_device(
                    &vello::wgpu::DeviceDescriptor {
                        label: None,
                        required_features: vello::wgpu::Features::empty(),
                        required_limits: vello::wgpu::Limits::downlevel_webgl2_defaults(),
                        memory_hints: vello::wgpu::MemoryHints::Performance,
                    },
                    None,
                )
                .await
                .map_err(|e| JsValue::from_str(&format!("failed to request device: {e}")))?;

            let renderer = VelloRenderer::init_surface(
                &adapter,
                device,
                queue,
                surface,
                logical_width,
                logical_height,
                dpr,
            );

            let mut app = TurApp::new(renderer)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            app.set_size(logical_width as f64, logical_height as f64);

            let resize_state = state_clone.clone();
            let resize_closure = Closure::<dyn Fn()>::new(move || {
                let _ = Self::handle_resize(&resize_state);
            });

            window
                .add_event_listener_with_callback("resize", resize_closure.as_ref().unchecked_ref())
                .err_to_jsval()?;

            let wasm_state = WasmState {
                app,
                canvas,
                _resize_closure: resize_closure,
            };

            *state_clone.borrow_mut() = Some(wasm_state);

            Ok(JsValue::undefined())
        })
    }

    pub fn load_and_run_js(&mut self, js_source: &str) -> Result<(), JsValue> {
        let mut guard = self.state.borrow_mut();
        let state = guard
            .as_mut()
            .ok_or_else(|| JsValue::from_str("app not initialized"))?;
        state
            .app
            .load_js(js_source)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        state.app.render();
        state
            .app
            .renderer_mut()
            .present()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(())
    }
}

impl TurWasmApp {
    fn handle_resize(state: &Rc<RefCell<Option<WasmState>>>) -> Result<(), JsValue> {
        let mut guard = state.borrow_mut();
        let state = guard
            .as_mut()
            .ok_or_else(|| JsValue::from_str("app not initialized"))?;

        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let dpr = window.device_pixel_ratio();
        let logical_width = window
            .inner_width()
            .err_to_jsval()?
            .as_f64()
            .unwrap_or(800.0) as u32;
        let logical_height = window
            .inner_height()
            .err_to_jsval()?
            .as_f64()
            .unwrap_or(600.0) as u32;

        let physical_width = (logical_width as f64 * dpr) as u32;
        let physical_height = (logical_height as f64 * dpr) as u32;

        state.canvas.set_width(physical_width);
        state.canvas.set_height(physical_height);

        state
            .app
            .renderer_mut()
            .resize(logical_width, logical_height, dpr);
        state.app.set_size(logical_width as f64, logical_height as f64);
        state.app.render();
        state
            .app
            .renderer_mut()
            .present()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(())
    }
}
