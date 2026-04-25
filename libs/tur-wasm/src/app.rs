use std::cell::RefCell;
use std::rc::Rc;
use tur_engine::TurApp;
use tur_engine::core::event::RawAppEvent;
use tur_engine::core::fonts::PresetFontLoader;
use tur_engine::renderer::vello::VelloRenderer;
use tur_shared::Offset;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::future_to_promise;

struct WasmState {
    app: TurApp,
    _canvas: web_sys::HtmlCanvasElement,
    _resize_closure: Closure<dyn Fn()>,
    _pointer_down_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _pointer_up_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
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

            let mut app = TurApp::new(
                Box::new(renderer),
                Box::new(PresetFontLoader::new()),
            )
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
            app.set_size(logical_width as f64, logical_height as f64);

            let resize_state = state_clone.clone();
            let resize_closure = Closure::<dyn Fn()>::new(move || {
                let _ = Self::handle_resize(&resize_state);
            });

            window
                .add_event_listener_with_callback("resize", resize_closure.as_ref().unchecked_ref())
                .err_to_jsval()?;

            let pointer_down_state = state_clone.clone();
            let pointer_down_closure =
                Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                    let mut guard = pointer_down_state.borrow_mut();
                    if let Some(s) = guard.as_mut() {
                        let rect = s._canvas.get_bounding_client_rect();
                        let x = event.client_x() as f64 - rect.left();
                        let y = event.client_y() as f64 - rect.top();
                        s.app
                            .dispatch_raw_event(RawAppEvent::PointerDown {
                                position: Offset::new(x, y),
                            });
                    }
                });

            canvas
                .add_event_listener_with_callback(
                    "mousedown",
                    pointer_down_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let pointer_up_state = state_clone.clone();
            let pointer_up_closure =
                Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                    let mut guard = pointer_up_state.borrow_mut();
                    if let Some(s) = guard.as_mut() {
                        let rect = s._canvas.get_bounding_client_rect();
                        let x = event.client_x() as f64 - rect.left();
                        let y = event.client_y() as f64 - rect.top();
                        s.app
                            .dispatch_raw_event(RawAppEvent::PointerUp {
                                position: Offset::new(x, y),
                            });
                    }
                });

            canvas
                .add_event_listener_with_callback(
                    "mouseup",
                    pointer_up_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let wasm_state = WasmState {
                app,
                _canvas: canvas,
                _resize_closure: resize_closure,
                _pointer_down_closure: pointer_down_closure,
                _pointer_up_closure: pointer_up_closure,
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

        state._canvas.set_width(physical_width);
        state._canvas.set_height(physical_height);

        state
            .app
            .renderer_resize(logical_width, logical_height, dpr);
        state.app.set_size(logical_width as f64, logical_height as f64);
        state.app.render();
        state
            .app
            .present()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(())
    }
}
