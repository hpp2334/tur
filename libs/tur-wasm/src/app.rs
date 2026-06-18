use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tur_engine::TurApp;
use tur_engine::core::event::{AppEvent, AppGestureEvent, AppImeEvent};
use tur_engine::core::fonts::PresetFontLoader;
use tur_engine::core::keyboard::{AppKeyEvent, KeyEventType, Modifiers};
use tur_engine::renderer::vello::VelloRenderer;
use tur_shared::Offset;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::future_to_promise;

struct WasmState {
    app: TurApp,
    _canvas: web_sys::HtmlCanvasElement,
    textarea: web_sys::HtmlTextAreaElement,
    is_composing: Cell<bool>,
    _resize_closure: Closure<dyn Fn()>,
    _pointer_down_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _pointer_up_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _pointer_move_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _wheel_closure: Closure<dyn Fn(web_sys::WheelEvent)>,
    _keydown_closure: Closure<dyn Fn(web_sys::KeyboardEvent)>,
    _keyup_closure: Closure<dyn Fn(web_sys::KeyboardEvent)>,
    _compositionstart_closure: Closure<dyn Fn(web_sys::CompositionEvent)>,
    _compositionupdate_closure: Closure<dyn Fn(web_sys::CompositionEvent)>,
    _compositionend_closure: Closure<dyn Fn(web_sys::CompositionEvent)>,
    _raf_closure: RefCell<Option<Closure<dyn Fn()>>>,
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

/// Register the swc-backed compiler services as `globalThis.__turHost`.
/// `transpileTsx(src: string): string` (throws on parse error) and
/// `tokenizeTsx(src: string): Array<{ start, end, kind }>` (coarse token
/// categories for syntax highlighting).
fn register_host_services(app: &mut TurApp) {
    use boa_engine::native_function::NativeFunction;
    use boa_engine::object::builtins::JsArray;
    use boa_engine::object::JsObject;
    use boa_engine::{js_string, JsArgs, JsError, JsNativeError, JsValue};

    let transpile = NativeFunction::from_copy_closure(|_this, args, _ctx| {
        let src = args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| {
                JsError::from(JsNativeError::typ().with_message("transpileTsx: expected a string"))
            })?
            .to_std_string_escaped();
        match crate::compiler::transpile_tsx(&src) {
            Ok(code) => Ok(JsValue::from(js_string!(code))),
            Err(e) => Err(JsError::from(JsNativeError::typ().with_message(e))),
        }
    });
    if let Err(e) = app.register_host_fn("transpileTsx", 1, transpile) {
        tracing::error!("failed to register transpileTsx: {e}");
    }

    let tokenize = NativeFunction::from_copy_closure(|_this, args, ctx| {
        let src = args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| {
                JsError::from(JsNativeError::typ().with_message("tokenizeTsx: expected a string"))
            })?
            .to_std_string_escaped();
        let spans = crate::compiler::tokenize_tsx(&src);
        let arr = JsArray::new(ctx)?;
        for sp in spans {
            let obj = JsObject::with_object_proto(ctx.intrinsics());
            obj.create_data_property(js_string!("start"), JsValue::from(sp.start as f64), ctx)?;
            obj.create_data_property(js_string!("end"), JsValue::from(sp.end as f64), ctx)?;
            obj.create_data_property(js_string!("kind"), JsValue::from(sp.kind as f64), ctx)?;
            arr.push(obj, ctx)?;
        }
        Ok(arr.into())
    });
    if let Err(e) = app.register_host_fn("tokenizeTsx", 1, tokenize) {
        tracing::error!("failed to register tokenizeTsx: {e}");
    }
}

#[wasm_bindgen]
impl TurWasmApp {
    pub fn create() -> js_sys::Promise {
        Self::create_internal(None)
    }

    pub fn create_in(container_id: String) -> js_sys::Promise {
        Self::create_internal(Some(container_id))
    }

    fn create_internal(container_id: Option<String>) -> js_sys::Promise {
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

            let container: web_sys::HtmlElement = if let Some(ref id) = container_id {
                document
                    .get_element_by_id(id)
                    .ok_or_else(|| JsValue::from_str(&format!("element #{id} not found")))?
                    .dyn_into()
                    .err_to_jsval()?
            } else {
                document.body().ok_or_else(|| JsValue::from_str("no body"))?
            };

            container.append_child(&canvas).err_to_jsval()?;

            if container_id.is_some() {
                canvas
                    .style()
                    .set_property("width", "100%")
                    .err_to_jsval()?;
                canvas
                    .style()
                    .set_property("height", "100%")
                    .err_to_jsval()?;
                canvas
                    .style()
                    .set_property("display", "block")
                    .err_to_jsval()?;
            } else {
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
                let body = document.body().ok_or_else(|| JsValue::from_str("no body"))?;
                body.style()
                    .set_property("margin", "0")
                    .err_to_jsval()?;
                body.style()
                    .set_property("overflow", "hidden")
                    .err_to_jsval()?;
            }

            let textarea = document
                .create_element("textarea")
                .err_to_jsval()?
                .dyn_into::<web_sys::HtmlTextAreaElement>()
                .err_to_jsval()?;

            textarea
                .style()
                .set_property("position", "absolute")
                .err_to_jsval()?;
            textarea
                .style()
                .set_property("opacity", "0")
                .err_to_jsval()?;
            textarea
                .style()
                .set_property("width", "1px")
                .err_to_jsval()?;
            textarea
                .style()
                .set_property("height", "1px")
                .err_to_jsval()?;
            textarea
                .style()
                .set_property("overflow", "hidden")
                .err_to_jsval()?;
            textarea
                .style()
                .set_property("border", "none")
                .err_to_jsval()?;
            textarea
                .style()
                .set_property("padding", "0")
                .err_to_jsval()?;
            textarea
                .style()
                .set_property("outline", "none")
                .err_to_jsval()?;
            textarea
                .set_attribute("autocomplete", "off")
                .err_to_jsval()?;
            textarea
                .set_attribute("autocorrect", "off")
                .err_to_jsval()?;
            textarea
                .set_attribute("autocapitalize", "off")
                .err_to_jsval()?;
            textarea
                .set_attribute("spellcheck", "false")
                .err_to_jsval()?;
            document.body().ok_or_else(|| JsValue::from_str("no body"))?.append_child(&textarea).err_to_jsval()?;

            let (logical_width, logical_height) = if container_id.is_some() {
                let rect = container.get_bounding_client_rect();
                (rect.width() as u32, rect.height() as u32)
            } else {
                let w = window
                    .inner_width()
                    .err_to_jsval()?
                    .as_f64()
                    .unwrap_or(800.0) as u32;
                let h = window
                    .inner_height()
                    .err_to_jsval()?
                    .as_f64()
                    .unwrap_or(600.0) as u32;
                (w, h)
            };
            let dpr = window.device_pixel_ratio();

            let physical_width = (logical_width as f64 * dpr) as u32;
            let physical_height = (logical_height as f64 * dpr) as u32;
            canvas.set_width(physical_width);
            canvas.set_height(physical_height);

            let instance = vello::wgpu::Instance::new(vello::wgpu::InstanceDescriptor {
                backends: vello::wgpu::Backends::BROWSER_WEBGPU,
                flags: vello::wgpu::InstanceFlags::default(),
                memory_budget_thresholds: vello::wgpu::MemoryBudgetThresholds::default(),
                backend_options: vello::wgpu::BackendOptions::default(),
                display: None,
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
                .map_err(|e| JsValue::from_str(&format!("failed to request adapter: {e}")))?;

            let (device, queue) = adapter
                .request_device(&vello::wgpu::DeviceDescriptor {
                    label: None,
                    required_features: vello::wgpu::Features::empty(),
                    required_limits: vello::wgpu::Limits::default(),
                    experimental_features: vello::wgpu::ExperimentalFeatures::default(),
                    memory_hints: vello::wgpu::MemoryHints::Performance,
                    trace: vello::wgpu::Trace::default(),
                })
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

            // Register the swc-backed compiler services on `globalThis.__turHost`
            // so JS (e.g. tur-demo-impl) can call `transpileTsx` / `tokenizeTsx`.
            // swc lives only in tur-wasm; tur-engine provides the generic hook.
            register_host_services(&mut app);

            app.push_event(AppEvent::Resize {
                logical_width,
                logical_height,
                dpr,
            });
            let _ = app.spawn_loop_once(std::time::Duration::ZERO);

            let resize_state = state_clone.clone();
            let resize_container_id = container_id.clone();
            let resize_closure = Closure::<dyn Fn()>::new(move || {
                let guard = resize_state.borrow();
                if let Some(s) = guard.as_ref() {
                    let window = web_sys::window().unwrap();
                    let dpr = window.device_pixel_ratio();
                    let (logical_width, logical_height) = if resize_container_id.is_some() {
                        let document = window.document().unwrap();
                        if let Some(el) = resize_container_id.as_ref().and_then(|id| document.get_element_by_id(id)) {
                            let rect = el.get_bounding_client_rect();
                            (rect.width() as u32, rect.height() as u32)
                        } else {
                            return;
                        }
                    } else {
                        let w = window.inner_width().unwrap().as_f64().unwrap_or(800.0) as u32;
                        let h = window.inner_height().unwrap().as_f64().unwrap_or(600.0) as u32;
                        (w, h)
                    };
                    let physical_width = (logical_width as f64 * dpr) as u32;
                    let physical_height = (logical_height as f64 * dpr) as u32;
                    s._canvas.set_width(physical_width);
                    s._canvas.set_height(physical_height);
                    s.app.push_event(AppEvent::Resize {
                        logical_width,
                        logical_height,
                        dpr,
                    });
                }
            });

            window
                .add_event_listener_with_callback("resize", resize_closure.as_ref().unchecked_ref())
                .err_to_jsval()?;

            let pointer_down_state = state_clone.clone();
            let pointer_down_closure =
                Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                    let guard = pointer_down_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        let rect = s._canvas.get_bounding_client_rect();
                        let x = event.client_x() as f64 - rect.left();
                        let y = event.client_y() as f64 - rect.top();
                        s.app.push_event(AppEvent::Gesture(
                            AppGestureEvent::PointerDown {
                                position: Offset::new(x, y),
                            },
                        ));
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
                    let guard = pointer_up_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        let rect = s._canvas.get_bounding_client_rect();
                        let x = event.client_x() as f64 - rect.left();
                        let y = event.client_y() as f64 - rect.top();
                        s.app.push_event(AppEvent::Gesture(
                            AppGestureEvent::PointerUp {
                                position: Offset::new(x, y),
                            },
                        ));
                    }
                });

            canvas
                .add_event_listener_with_callback(
                    "mouseup",
                    pointer_up_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let pointer_move_state = state_clone.clone();
            let pointer_move_closure =
                Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                    let guard = pointer_move_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        let rect = s._canvas.get_bounding_client_rect();
                        let x = event.client_x() as f64 - rect.left();
                        let y = event.client_y() as f64 - rect.top();
                        s.app.push_event(AppEvent::Gesture(
                            AppGestureEvent::PointerMove {
                                position: Offset::new(x, y),
                            },
                        ));
                    }
                });

            canvas
                .add_event_listener_with_callback(
                    "mousemove",
                    pointer_move_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let wheel_state = state_clone.clone();
            let wheel_closure =
                Closure::<dyn Fn(web_sys::WheelEvent)>::new(move |event: web_sys::WheelEvent| {
                    event.prevent_default();
                    let guard = wheel_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        let rect = s._canvas.get_bounding_client_rect();
                        let x = event.client_x() as f64 - rect.left();
                        let y = event.client_y() as f64 - rect.top();
                        s.app.push_event(AppEvent::Wheel {
                            delta_x: event.delta_x(),
                            delta_y: event.delta_y(),
                            position: Offset::new(x, y),
                        });
                    }
                });

            canvas
                .add_event_listener_with_callback(
                    "wheel",
                    wheel_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            canvas
                .set_attribute("tabindex", "0")
                .err_to_jsval()?;
            canvas
                .style()
                .set_property("outline", "none")
                .err_to_jsval()?;

            let keydown_state = state_clone.clone();
            let keydown_closure =
                Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
                    let guard = keydown_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        if s.is_composing.get() {
                            return;
                        }
                        s.app.push_event(AppEvent::Key(AppKeyEvent {
                            key: event.key(),
                            code: event.code(),
                            modifiers: Modifiers {
                                ctrl: event.ctrl_key(),
                                shift: event.shift_key(),
                                alt: event.alt_key(),
                                meta: event.meta_key(),
                            },
                            event_type: KeyEventType::Down,
                        }));
                    }
                });

            canvas
                .add_event_listener_with_callback(
                    "keydown",
                    keydown_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            textarea
                .add_event_listener_with_callback(
                    "keydown",
                    keydown_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let keyup_state = state_clone.clone();
            let keyup_closure =
                Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
                    let guard = keyup_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        if s.is_composing.get() {
                            return;
                        }
                        s.app.push_event(AppEvent::Key(AppKeyEvent {
                            key: event.key(),
                            code: event.code(),
                            modifiers: Modifiers {
                                ctrl: event.ctrl_key(),
                                shift: event.shift_key(),
                                alt: event.alt_key(),
                                meta: event.meta_key(),
                            },
                            event_type: KeyEventType::Up,
                        }));
                    }
                });

            canvas
                .add_event_listener_with_callback(
                    "keyup",
                    keyup_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            textarea
                .add_event_listener_with_callback(
                    "keyup",
                    keyup_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let comp_start_state = state_clone.clone();
            let compositionstart_closure =
                Closure::<dyn Fn(web_sys::CompositionEvent)>::new(move |_event: web_sys::CompositionEvent| {
                    let guard = comp_start_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        s.is_composing.set(true);
                        s.app.push_event(AppEvent::Ime(
                            AppImeEvent::CompositionStart,
                        ));
                    }
                });

            textarea
                .add_event_listener_with_callback(
                    "compositionstart",
                    compositionstart_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let comp_update_state = state_clone.clone();
            let compositionupdate_closure =
                Closure::<dyn Fn(web_sys::CompositionEvent)>::new(move |event: web_sys::CompositionEvent| {
                    let guard = comp_update_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        let text = event.data().unwrap_or_default();
                        s.app.push_event(AppEvent::Ime(
                            AppImeEvent::CompositionUpdate {
                                text,
                                cursor: None,
                            },
                        ));
                    }
                });

            textarea
                .add_event_listener_with_callback(
                    "compositionupdate",
                    compositionupdate_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let comp_end_state = state_clone.clone();
            let compositionend_closure =
                Closure::<dyn Fn(web_sys::CompositionEvent)>::new(move |event: web_sys::CompositionEvent| {
                    let guard = comp_end_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        s.is_composing.set(false);
                        let text = event.data().unwrap_or_default();
                        s.app.push_event(AppEvent::Ime(
                            AppImeEvent::CompositionEnd { text },
                        ));
                        s.textarea.set_value("");
                    }
                });

            textarea
                .add_event_listener_with_callback(
                    "compositionend",
                    compositionend_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let wasm_state = WasmState {
                app,
                _canvas: canvas,
                textarea,
                is_composing: Cell::new(false),
                _resize_closure: resize_closure,
                _pointer_down_closure: pointer_down_closure,
                _pointer_up_closure: pointer_up_closure,
                _pointer_move_closure: pointer_move_closure,
                _wheel_closure: wheel_closure,
                _keydown_closure: keydown_closure,
                _keyup_closure: keyup_closure,
                _compositionstart_closure: compositionstart_closure,
                _compositionupdate_closure: compositionupdate_closure,
                _compositionend_closure: compositionend_closure,
                _raf_closure: RefCell::new(None),
            };

            *state_clone.borrow_mut() = Some(wasm_state);

            let app = TurWasmApp { state: state_clone };
            Ok(JsValue::from(app))
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

        state.app.push_event(AppEvent::RequestDraw);
        if let Err(e) = state.app.spawn_loop_once(std::time::Duration::ZERO) {
            tracing::error!("load_and_run_js: initial spawn_loop_once error: {e}");
        }

        drop(guard);
        Self::start_frame_loop(&self.state);

        Ok(())
    }

    pub fn debug_layout(&self) -> String {
        let guard = self.state.borrow();
        match guard.as_ref() {
            Some(s) => s.app.debug_layout(),
            None => "app not initialized".to_string(),
        }
    }
}

impl TurWasmApp {
    fn start_frame_loop(state: &Rc<RefCell<Option<WasmState>>>) {
        let loop_state = state.clone();
        let raf_closure = Closure::<dyn Fn()>::new(move || {
            let mut guard = loop_state.borrow_mut();
            if let Some(s) = guard.as_mut() {
                if let Err(e) = s.app.spawn_loop_once(std::time::Duration::from_millis(16)) {
                    tracing::error!("frame loop spawn_loop_once error: {e}");
                }

                let is_editable = s.app.focused_is_editable();
                if is_editable {
                    let _ = s.textarea.focus();
                    if let Some((x, y, _w, _h)) = s.app.focused_cursor_rect() {
                        let _ = s.textarea.style().set_property("left", &format!("{x}px"));
                        let _ = s.textarea.style().set_property("top", &format!("{y}px"));
                    }
                }

                drop(guard);
                Self::start_frame_loop(&loop_state);
            }
        });

        let window = web_sys::window().unwrap();
        let _ = window.request_animation_frame(raf_closure.as_ref().unchecked_ref());

        let guard = state.borrow();
        if let Some(s) = guard.as_ref() {
            *s._raf_closure.borrow_mut() = Some(raf_closure);
        }
    }
}
