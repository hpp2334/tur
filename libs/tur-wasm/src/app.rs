use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use boa_engine::context::time::{Clock, JsInstant};
use tur_engine::core::app::NextFrame;
use tur_engine::core::event::{AppImeEvent, PlatformEvent, PointerInput};
use crate::fonts::WasmFontLoader;
use tur_engine::core::keyboard::{AppKeyEvent, KeyEventType, Modifiers};
use tur_engine::renderer::vello::WebGlVelloRenderer;
use tur_engine::{CursorCap, LoopDriver, TurApp};
use tur_shared::Offset;
use tur_clipboard_wasm::{Clipboard, TurClipboardPlugin, WasmClipboard};
use tur_net_wasm::{Http, TurNetPlugin, WasmHttp};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::future_to_promise;

/// Engine `Clock` for wasm. boa's `StdClock` panics on
/// `wasm32-unknown-unknown` (`SystemTime::now()` is unimplemented), and
/// `std::time::Instant::now()` is unsupported too, so this reads
/// `Date.now()` — the same wall-clock source the old frame loop used for its
/// per-frame delta. Production thus gets live real time with no manual clock
/// forwarding. (Not strictly monotonic across system-clock adjustments, but
/// the engine's animation/timer math derives durations from deltas, which is
/// robust to the rare jump.)
#[derive(Default)]
struct WasmClock;

impl Clock for WasmClock {
    fn now(&self) -> JsInstant {
        let ms = js_sys::Date::now();
        let secs = (ms / 1000.0) as u64;
        let nanos = ((ms % 1000.0) * 1_000_000.0) as u32;
        JsInstant::new(secs, nanos)
    }

    fn system_time_millis(&self) -> i64 {
        js_sys::Date::now() as i64
    }
}

struct WasmState {
    app: Rc<TurApp>,
    _canvas: web_sys::HtmlCanvasElement,
    textarea: web_sys::HtmlTextAreaElement,
    is_composing: Cell<bool>,
    _resize_closure: Closure<dyn Fn()>,
    _resize_observer: web_sys::ResizeObserver,
    _pointer_down_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _pointer_up_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _pointer_move_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _wheel_closure: Closure<dyn Fn(web_sys::WheelEvent)>,
    _context_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _touch_start_closure: Closure<dyn Fn(web_sys::TouchEvent)>,
    _touch_move_closure: Closure<dyn Fn(web_sys::TouchEvent)>,
    _touch_end_closure: Closure<dyn Fn(web_sys::TouchEvent)>,
    _touch_cancel_closure: Closure<dyn Fn(web_sys::TouchEvent)>,
    _keydown_closure: Closure<dyn Fn(web_sys::KeyboardEvent)>,
    _keyup_closure: Closure<dyn Fn(web_sys::KeyboardEvent)>,
    _compositionstart_closure: Closure<dyn Fn(web_sys::CompositionEvent)>,
    _compositionupdate_closure: Closure<dyn Fn(web_sys::CompositionEvent)>,
    _compositionend_closure: Closure<dyn Fn(web_sys::CompositionEvent)>,
    _paste_closure: Closure<dyn Fn(web_sys::ClipboardEvent)>,
}

/// Embedder-side `CursorBackend`: the engine pushes the resolved cursor here
/// during the frame loop, and we apply it to the host canvas.
struct WasmCursor {
    canvas: web_sys::HtmlCanvasElement,
}

impl tur_engine::CursorBackend for WasmCursor {
    fn set_cursor(&mut self, cursor: tur_shared::Cursor) {
        let _ = self.canvas.style().set_property("cursor", cursor.as_str());
    }
}

// `Clipboard` impl lives in `tur-clipboard-wasm` now — re-exported here via
// `use tur_clipboard_wasm::{Clipboard, WasmClipboard}`.

// `Http` impl lives in `tur-net-wasm` now — re-exported here via
// `use tur_net_wasm::{Http, WasmHttp}`.

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

/// Translate a DOM `MouseEvent.button` (+ ctrlKey state) into our
/// [`MouseButton`]. On macOS, a context-menu is triggered by Ctrl+click,
/// which the browser reports as `button=0` (primary) with `ctrlKey=true`;
/// normalize that to [`MouseButton::Right`] so the engine's arena derives a
/// context-menu gesture from the resulting right-button pointer up.
fn normalize_mouse_button(dom_button: u16, ctrl_key: bool) -> tur_shared::MouseButton {
    let button = tur_shared::MouseButton::from_dom(dom_button);
    if ctrl_key && button == tur_shared::MouseButton::Left {
        tur_shared::MouseButton::Right
    } else {
        button
    }
}

/// Build the swc-backed compiler + clipboard host functions.
///
/// Returns `(name, fn, length)` tuples. The caller registers them both as the
/// legacy `globalThis.__turHost.*` globals and as the `builtin:tur/host` module
/// exports (see [`register_all_services`]).
///
/// - `transpileTsx(src): string` (throws on parse error)
/// - `tokenizeTsx(src): Array<{ start, end, kind }>` (lexical token categories
///   refined by AST-derived semantic categories — declaration names, JSX
///   tags/attributes, type names, comments — for syntax highlighting)
/// - `generateAst(src): AstNode[]`
/// - `clipboardWriteText(text)` / `clipboardReadText(callback)`
fn build_host_service_fns() -> Vec<(&'static str, boa_engine::NativeFunction, usize)> {
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

    let tokenize = NativeFunction::from_copy_closure(|_this, args, ctx| {
        let src = args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| {
                JsError::from(JsNativeError::typ().with_message("tokenizeTsx: expected a string"))
            })?
            .to_std_string_escaped();
        let spans = crate::compiler::highlight_tsx(&src);
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

    let generate_ast = NativeFunction::from_copy_closure(|_this, args, ctx| {        let src = args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| {
                JsError::from(JsNativeError::typ().with_message("generateAst: expected a string"))
            })?
            .to_std_string_escaped();
        let nodes = crate::compiler::generate_ast(&src)
            .map_err(|e| JsError::from(JsNativeError::typ().with_message(e)))?;

        let arr = JsArray::new(ctx)?;
        for node in nodes {
            let obj = JsObject::with_object_proto(ctx.intrinsics());
            let kind_str = match &node.kind {
                crate::compiler::AstNodeKind::Import { .. } => "import",
                crate::compiler::AstNodeKind::ExportDecl { .. } => "exportDecl",
                crate::compiler::AstNodeKind::ExportDefault => "exportDefault",
                crate::compiler::AstNodeKind::ExportNamed { .. } => "exportNamed",
                crate::compiler::AstNodeKind::ExportAll => "exportAll",
                crate::compiler::AstNodeKind::ExportType { .. } => "exportType",
                crate::compiler::AstNodeKind::Statement => "statement",
            };
            obj.create_data_property(js_string!("kind"), JsValue::from(js_string!(kind_str)), ctx)?;
            obj.create_data_property(js_string!("text"), JsValue::from(js_string!(node.text.as_str())), ctx)?;
            if let Some(body) = &node.body {
                obj.create_data_property(js_string!("body"), JsValue::from(js_string!(body.as_str())), ctx)?;
            }

            match &node.kind {
                crate::compiler::AstNodeKind::Import { source, specifiers } => {
                    obj.create_data_property(js_string!("source"), JsValue::from(js_string!(source.as_str())), ctx)?;
                    let spec_arr = JsArray::new(ctx)?;
                    for spec in specifiers {
                        let spec_obj = JsObject::with_object_proto(ctx.intrinsics());
                        spec_obj.create_data_property(js_string!("local"), JsValue::from(js_string!(spec.local.as_str())), ctx)?;
                        spec_obj.create_data_property(js_string!("imported"), JsValue::from(js_string!(spec.imported.as_str())), ctx)?;
                        spec_arr.push(spec_obj, ctx)?;
                    }
                    obj.create_data_property(js_string!("specifiers"), JsValue::from(spec_arr), ctx)?;
                }
                crate::compiler::AstNodeKind::ExportDecl { names }
                | crate::compiler::AstNodeKind::ExportNamed { names }
                | crate::compiler::AstNodeKind::ExportType { names } => {
                    let name_arr = JsArray::new(ctx)?;
                    for n in names {
                        name_arr.push(JsValue::from(js_string!(n.as_str())), ctx)?;
                    }
                    obj.create_data_property(js_string!("names"), JsValue::from(name_arr), ctx)?;
                }
                _ => {}
            }

            arr.push(obj, ctx)?;
        }
        Ok(arr.into())
    });

    vec![
        ("transpileTsx", transpile, 1),
        ("tokenizeTsx", tokenize, 1),
        ("generateAst", generate_ast, 1),
    ]
}

// ---------------------------------------------------------------------------
// `perform_request` moved to `tur-net-wasm/src/backend.rs` — extracted
// verbatim so the WasmHttp backend lives in its own crate.
// ---------------------------------------------------------------------------

/// A pending `__turHost.pickFile` result: callback + (`None` if cancelled).
type PendingPick = (
    boa_engine::object::builtins::JsFunction,
    Option<(String, Vec<u8>)>,
);

/// The `change` handler closure type used by the hidden file-picker `<input>`.
type PickHandler = wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>;


// --- File IO host bridges (browser-only) -----------------------------------
//
// `pickFile(callback)` opens the native file picker and resolves with
// `{ name, bytes<ArrayBuffer> }` (or null if cancelled). `saveFile(name, bytes)`
// triggers a browser download. Bytes round-trip through boa ArrayBuffers; the
// actual File/Blob live in the browser heap, so we copy through `Vec<u8>`.

fn build_file_io_fns() -> Vec<(&'static str, boa_engine::NativeFunction, usize)> {
    use boa_engine::native_function::NativeFunction;
    use boa_engine::object::builtins::{JsArrayBuffer, JsFunction};
    use boa_engine::{JsArgs, JsValue};
    use wasm_bindgen::JsCast;

    let pick_file = NativeFunction::from_copy_closure(move |_this, args, _ctx| {
        let Some(cb_obj) = args.get_or_undefined(0).as_object() else {
            return Ok(JsValue::undefined());
        };
        let Some(cb) = JsFunction::from_object(cb_obj.clone()) else {
            return Ok(JsValue::undefined());
        };
        let Some(window) = web_sys::window() else {
            return Ok(JsValue::undefined());
        };
        let Some(document) = window.document() else {
            return Ok(JsValue::undefined());
        };
        let Ok(input_el) = document.create_element("input") else {
            return Ok(JsValue::undefined());
        };
        let Ok(input) = input_el.dyn_into::<web_sys::HtmlInputElement>() else {
            return Ok(JsValue::undefined());
        };
        input.set_type("file");

        let input_for_handler = input.clone();
        let cb_for_handler = cb.clone();
        let on_change = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev| {
            let picked = input_for_handler.files().and_then(|fl| fl.get(0));
            match picked {
                Some(file) => {
                    let name = file.name();
                    let cb2 = cb_for_handler.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let bytes = match wasm_bindgen_futures::JsFuture::from(file.array_buffer())
                            .await
                        {
                            Ok(buf) => {
                                let arr = js_sys::Uint8Array::new(&buf);
                                let mut v = vec![0u8; arr.length() as usize];
                                arr.copy_to(&mut v);
                                Some(v)
                            }
                            Err(_) => None,
                        };
                        FILE_PICK_RESULTS
                            .with(|q| q.borrow_mut().push((cb2, bytes.map(|b| (name, b)))));
                    });
                }
                None => {
                    FILE_PICK_RESULTS
                        .with(|q| q.borrow_mut().push((cb_for_handler.clone(), None)));
                }
            }
        });
        input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
        PICK_CLOSURES.with(|c| c.borrow_mut().push(on_change));
        input.click();
        Ok(JsValue::undefined())
    });

    let save_file = NativeFunction::from_copy_closure(move |_this, args, _ctx| {
        let name = args
            .get_or_undefined(0)
            .as_string()
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|| "download".to_string());
        let bytes: Option<Vec<u8>> = args
            .get_or_undefined(1)
            .as_object()
            .and_then(|o| JsArrayBuffer::from_object(o.clone()).ok())
            .and_then(|ab| ab.to_vec());
        if let (Some(bytes), Some(window)) = (bytes, web_sys::window()) {
            let document = window.document();
            let arr = js_sys::Uint8Array::from(&bytes[..]);
            let parts = js_sys::Array::new();
            parts.push(&arr);
            if let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence(&parts) {
                if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                    if let Some(document) = document {
                        if let Ok(a_el) = document.create_element("a") {
                            if let Ok(a) = a_el.dyn_into::<web_sys::HtmlAnchorElement>() {
                                a.set_href(&url);
                                a.set_download(&name);
                                if let Some(body) = document.body() {
                                    let _ = body.append_child(&a);
                                    a.click();
                                    let _ = body.remove_child(&a);
                                }
                            }
                        }
                    }
                    let _ = web_sys::Url::revoke_object_url(&url);
                }
            }
        }
        Ok(JsValue::undefined())
    });

    vec![
        ("pickFile", pick_file, 1),
        ("saveFile", save_file, 2),
    ]
}

thread_local! {
    /// Pending `(callback, picked-file)` pairs queued by `__turHost.pickFile`
    /// once the browser File bytes are read. `None` = picker cancelled.
    static FILE_PICK_RESULTS: std::cell::RefCell<Vec<PendingPick>>
        = const { std::cell::RefCell::new(Vec::new()) };

    /// Keeps the per-pick `change` closures alive for the lifetime of the
    /// hidden `<input type=file>` (otherwise they'd be dropped before the user
    /// selects a file). Accumulates; entries outlive their use but the leak is
    /// negligible for a playground.
    static PICK_CLOSURES: std::cell::RefCell<Vec<PickHandler>>
        = const { std::cell::RefCell::new(Vec::new()) };
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

            if container_id.is_some() {
                container.append_child(&canvas).err_to_jsval()?;
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
                // Full-viewport mode: wrap the canvas in a div that owns the
                // safe, visible viewport. The wrapper is sized in dynamic
                // viewport units (`100dvw`/`100dvh` — fall back to `vw`/`vh`
                // where unsupported) so it shrinks with the mobile browser's
                // sliding URL bar, and its `env(safe-area-inset-*)` padding
                // (`box-sizing: border-box`) insets the content box past the
                // device notch / home indicator. The canvas fills the content
                // box, so the engine viewport == the non-occluded render area
                // — safe-area stays transparent to the app.
                let wrapper = document
                    .create_element("div")
                    .err_to_jsval()?
                    .dyn_into::<web_sys::HtmlElement>()
                    .err_to_jsval()?;
                let ws = wrapper.style();
                ws.set_property("position", "fixed").err_to_jsval()?;
                ws.set_property("top", "0").err_to_jsval()?;
                ws.set_property("left", "0").err_to_jsval()?;
                ws.set_property("width", "100vw").err_to_jsval()?;
                ws.set_property("width", "100dvw").err_to_jsval()?;
                ws.set_property("height", "100vh").err_to_jsval()?;
                ws.set_property("height", "100dvh").err_to_jsval()?;
                ws.set_property("box-sizing", "border-box").err_to_jsval()?;
                ws.set_property("padding-top", "env(safe-area-inset-top)")
                    .err_to_jsval()?;
                ws.set_property("padding-right", "env(safe-area-inset-right)")
                    .err_to_jsval()?;
                ws.set_property("padding-bottom", "env(safe-area-inset-bottom)")
                    .err_to_jsval()?;
                ws.set_property("padding-left", "env(safe-area-inset-left)")
                    .err_to_jsval()?;
                // Match the app background (#fbfcfd = tokens.bg.app) so the
                // safe-area strip is seamless with the rendered canvas.
                ws.set_property("background", "#fbfcfd").err_to_jsval()?;
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
                let body = document.body().ok_or_else(|| JsValue::from_str("no body"))?;
                body.style()
                    .set_property("margin", "0")
                    .err_to_jsval()?;
                body.style()
                    .set_property("overflow", "hidden")
                    .err_to_jsval()?;
                body.append_child(&wrapper).err_to_jsval()?;
                wrapper.append_child(&canvas).err_to_jsval()?;
            }

            // Claim touch gestures for the app. With `touch-action: none` the
            // browser will not pan/zoom the page on touch-drag (we translate
            // touchmove → PlatformEvent::Wheel below). Taps still synthesize
            // mousedown/click for caret placement, buttons, and the soft
            // keyboard via the hidden textarea.
            canvas
                .style()
                .set_property("touch-action", "none")
                .err_to_jsval()?;

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

            // Measure the canvas's own rendered rect (CSS px). For full-viewport
            // mode this equals the wrapper's content box = the safe, visible
            // area (dvh minus safe-area padding); for container mode it's the
            // container's box. Either way the canvas — not `window.innerWidth` —
            // is the authoritative viewport.
            let (logical_width, logical_height) = if container_id.is_some() {
                let rect = container.get_bounding_client_rect();
                (rect.width() as u32, rect.height() as u32)
            } else {
                let rect = canvas.get_bounding_client_rect();
                (rect.width() as u32, rect.height() as u32)
            };
            let dpr = window.device_pixel_ratio();

            let physical_width = (logical_width as f64 * dpr) as u32;
            let physical_height = (logical_height as f64 * dpr) as u32;
            canvas.set_width(physical_width);
            canvas.set_height(physical_height);

            let renderer = WebGlVelloRenderer::new(
                canvas.clone(),
                logical_width,
                logical_height,
                dpr,
            );

            let host_fns = build_host_service_fns();
            let file_fns = build_file_io_fns();

            let host_exports: Vec<(String, boa_engine::NativeFunction, usize)> = host_fns
                .into_iter()
                .chain(file_fns)
                .map(|(n, f, l)| (n.to_string(), f, l))
                .collect();

            let app = tur_engine::TurEngine::builder()
                .renderer(Box::new(renderer))
                .font_loader(Box::new(WasmFontLoader::new()))
                .clock(Rc::new(WasmClock))
                .capability(CursorCap::new(WasmCursor {
                    canvas: canvas.clone(),
                }))
                .capability(Clipboard::new(WasmClipboard))
                .capability(Http::new(WasmHttp))
                .plugin(tur_std::TurStdPlugin)
                .plugin(TurClipboardPlugin)
                .plugin(TurNetPlugin)
                .host_module("builtin:tur/host", host_exports)
                .build()
                .map_err(|e| JsValue::from_str(&e.to_string()))?;

            app.push_platform_event(PlatformEvent::Resize {
                logical_width,
                logical_height,
                dpr,
            });

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
                        // Full-viewport mode: measure the canvas (== wrapper
                        // content box = safe area), not `window.innerHeight`.
                        let rect = s._canvas.get_bounding_client_rect();
                        (rect.width() as u32, rect.height() as u32)
                    };
                    let physical_width = (logical_width as f64 * dpr) as u32;
                    let physical_height = (logical_height as f64 * dpr) as u32;
                    s._canvas.set_width(physical_width);
                    s._canvas.set_height(physical_height);
                    s.app.push_platform_event(PlatformEvent::Resize {
                        logical_width,
                        logical_height,
                        dpr,
                    });
                }
            });

            // Observe the canvas directly. `ResizeObserver` fires whenever the
            // canvas border-box changes — covering window resize, orientation
            // change, *and* the mobile URL-bar slide (the wrapper's `dvh` height
            // changes → canvas resizes). `window` "resize" alone does not fire
            // on the URL-bar slide, so it is not used. The callback ignores its
            // entries argument and re-measures the canvas rect.
            let resize_observer = web_sys::ResizeObserver::new(
                resize_closure.as_ref().unchecked_ref(),
            )
            .err_to_jsval()?;
            resize_observer.observe(canvas.as_ref());

            let pointer_down_state = state_clone.clone();
            let pointer_down_closure =
                Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                    let guard = pointer_down_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        let rect = s._canvas.get_bounding_client_rect();
                        let x = event.client_x() as f64 - rect.left();
                        let y = event.client_y() as f64 - rect.top();
                        // Normalize macOS Ctrl+click (a primary-button press with
                        // Ctrl held) to a secondary/right button so the engine's
                        // gesture arena derives a context-menu from it. Other
                        // platforms don't send Ctrl+click for context-menu, so
                        // this only affects the macOS convention.
                        let button = normalize_mouse_button(event.button() as u16, event.ctrl_key());
                        // DOM `MouseEvent.timeStamp` is ms since epoch — used by
                        // the engine's gesture composer for multi-click
                        // (double/triple) classification.
                        let time_ms = event.time_stamp() as u64;
                        s.app.push_platform_event(PlatformEvent::Pointer(
                            PointerInput::PointerDown {
                                position: Offset::new(x, y),
                                button,
                                time_ms,
                                device: tur_engine::core::event::PointerDeviceKind::Mouse,
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
                        let button = normalize_mouse_button(event.button() as u16, event.ctrl_key());
                        s.app.push_platform_event(PlatformEvent::Pointer(
                            PointerInput::PointerUp {
                                position: Offset::new(x, y),
                                button,
                                device: tur_engine::core::event::PointerDeviceKind::Mouse,
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
                        s.app.push_platform_event(PlatformEvent::Pointer(
                            PointerInput::PointerMove {
                                position: Offset::new(x, y),
                                device: tur_engine::core::event::PointerDeviceKind::Mouse,
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
                        s.app.push_platform_event(PlatformEvent::Wheel {
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

            // Touch handling for mobile. Touch events are dispatched as
            // native touch pointer events (`device: Touch`) into the
            // engine's gesture arena, which resolves drag-vs-scroll
            // competition using a slop threshold (18px, matching Flutter's
            // `kTouchSlop`).
            //
            // - **touchstart**: push `PointerDown { device: Touch }`. The
            //   arena collects candidates from the hit-path but does NOT
            //   dispatch to elements yet. We do NOT `preventDefault` so
            //   that pure taps (no touchmove) still get browser-synthesized
            //   `mousedown`→`mouseup`→`click` for caret placement, button
            //   clicks, and soft-keyboard focus.
            //
            // - **touchmove**: `preventDefault` (stops browser panning AND
            //   mouse-event synthesis for the entire touch sequence). Push
            //   `PointerMove { device: Touch }`. The arena checks slop:
            //   if movement < 18px → no dispatch (still ambiguous); if ≥18px
            //   → resolve to drag winner (dispatch PointerDown+PointerMove)
            //   or scroll winner (dispatch Wheel).
            //
            // - **touchend**: push `PointerUp { device: Touch }`. If the
            //   arena resolved a drag → dispatch PointerUp to release
            //   capture. If not resolved but touchmove was seen (small
            //   movement) → synthesize a mouse click. If not resolved and
            //   no touchmove → do nothing (browser click handles it).
            //
            // - **touchcancel**: push `PointerCancel { device: Touch }`.
            //   The arena releases any captured drag without firing a click.
            let touch_start_state = state_clone.clone();
            let touch_start_closure =
                Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
                    let Some(t) = event.touches().get(0) else {
                        return;
                    };
                    let guard = touch_start_state.borrow();
                    let Some(s) = guard.as_ref() else {
                        return;
                    };
                    let rect = s._canvas.get_bounding_client_rect();
                    let x = t.client_x() as f64 - rect.left();
                    let y = t.client_y() as f64 - rect.top();
                    let time_ms = event.time_stamp() as u64;
                    s.app.push_platform_event(PlatformEvent::Pointer(
                        PointerInput::PointerDown {
                            position: Offset::new(x, y),
                            button: tur_shared::MouseButton::Left,
                            time_ms,
                            device: tur_engine::core::event::PointerDeviceKind::Touch,
                        },
                    ));
                });

            canvas
                .add_event_listener_with_callback(
                    "touchstart",
                    touch_start_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let touch_move_state = state_clone.clone();
            let touch_move_closure =
                Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
                    let Some(t) = event.touches().get(0) else {
                        return;
                    };
                    event.prevent_default();
                    let guard = touch_move_state.borrow();
                    let Some(s) = guard.as_ref() else {
                        return;
                    };
                    let rect = s._canvas.get_bounding_client_rect();
                    let x = t.client_x() as f64 - rect.left();
                    let y = t.client_y() as f64 - rect.top();
                    s.app.push_platform_event(PlatformEvent::Pointer(
                        PointerInput::PointerMove {
                            position: Offset::new(x, y),
                            device: tur_engine::core::event::PointerDeviceKind::Touch,
                        },
                    ));
                });

            canvas
                .add_event_listener_with_callback(
                    "touchmove",
                    touch_move_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let touch_end_state = state_clone.clone();
            let touch_end_closure =
                Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
                    let Some(t) = event.changed_touches().get(0) else {
                        let guard = touch_end_state.borrow();
                        if let Some(s) = guard.as_ref() {
                            s.app.push_platform_event(PlatformEvent::Pointer(
                                PointerInput::PointerUp {
                                    position: Offset::new(0.0, 0.0),
                                    button: tur_shared::MouseButton::Left,
                                    device: tur_engine::core::event::PointerDeviceKind::Touch,
                                },
                            ));
                        }
                        return;
                    };
                    let guard = touch_end_state.borrow();
                    let Some(s) = guard.as_ref() else {
                        return;
                    };
                    let rect = s._canvas.get_bounding_client_rect();
                    let x = t.client_x() as f64 - rect.left();
                    let y = t.client_y() as f64 - rect.top();
                    s.app.push_platform_event(PlatformEvent::Pointer(
                        PointerInput::PointerUp {
                            position: Offset::new(x, y),
                            button: tur_shared::MouseButton::Left,
                            device: tur_engine::core::event::PointerDeviceKind::Touch,
                        },
                    ));
                });

            canvas
                .add_event_listener_with_callback(
                    "touchend",
                    touch_end_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let touch_cancel_state = state_clone.clone();
            let touch_cancel_closure =
                Closure::<dyn Fn(web_sys::TouchEvent)>::new(move |_event: web_sys::TouchEvent| {
                    let guard = touch_cancel_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        s.app.push_platform_event(PlatformEvent::Pointer(
                            PointerInput::PointerCancel {
                                device: tur_engine::core::event::PointerDeviceKind::Touch,
                            },
                        ));
                    }
                });

            canvas
                .add_event_listener_with_callback(
                    "touchcancel",
                    touch_cancel_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            // Context menu listener. We only `preventDefault` to suppress the
            // native browser menu. The context-menu *gesture* itself is derived
            // inside the engine from the right-button `PointerUp` (the mouseup
            // listener above already pushes that — including macOS Ctrl+click,
            // which is normalized to a Right button). We must NOT push a
            // separate event here, otherwise a physical right-click would fire
            // context-menu twice (once from mouseup, once from here).
            let context_state = state_clone.clone();
            let context_closure =
                Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                    let _ = context_state;
                    event.prevent_default();
                });

            canvas
                .add_event_listener_with_callback(
                    "contextmenu",
                    context_closure.as_ref().unchecked_ref(),
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
                        s.app.push_platform_event(PlatformEvent::Key(AppKeyEvent {
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
                        s.app.push_platform_event(PlatformEvent::Key(AppKeyEvent {
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
                        s.app.push_platform_event(PlatformEvent::Ime(
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
                        s.app.push_platform_event(PlatformEvent::Ime(
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
                        s.app.push_platform_event(PlatformEvent::Ime(
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

            // Paste listener — when the user presses Cmd+V (or Ctrl+V) while
            // the hidden textarea is focused, the browser fires a `paste`
            // event with `clipboardData`. We forward the text to the engine
            // via PlatformEvent::ClipboardPaste, which the engine's
            // ClipboardPasteHandler inserts into the focused editable.
            let paste_state = state_clone.clone();
            let paste_closure =
                Closure::<dyn Fn(web_sys::ClipboardEvent)>::new(move |event: web_sys::ClipboardEvent| {
                    event.prevent_default();
                    let text = event
                        .clipboard_data()
                        .and_then(|d| d.get_data("text/plain").ok())
                        .unwrap_or_default();
                    if text.is_empty() {
                        return;
                    }
                    let guard = paste_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        s.app.push_platform_event(PlatformEvent::ClipboardPaste { text });
                    }
                });

            textarea
                .add_event_listener_with_callback(
                    "paste",
                    paste_closure.as_ref().unchecked_ref(),
                )
                .err_to_jsval()?;

            let wasm_state = WasmState {
                app,
                _canvas: canvas,
                textarea,
                is_composing: Cell::new(false),
                _resize_closure: resize_closure,
                _resize_observer: resize_observer,
                _pointer_down_closure: pointer_down_closure,
                _pointer_up_closure: pointer_up_closure,
                _pointer_move_closure: pointer_move_closure,
                _wheel_closure: wheel_closure,
                _context_closure: context_closure,
                _touch_start_closure: touch_start_closure,
                _touch_move_closure: touch_move_closure,
                _touch_end_closure: touch_end_closure,
                _touch_cancel_closure: touch_cancel_closure,
                _keydown_closure: keydown_closure,
                _keyup_closure: keyup_closure,
                _compositionstart_closure: compositionstart_closure,
                _compositionupdate_closure: compositionupdate_closure,
                _compositionend_closure: compositionend_closure,
                 _paste_closure: paste_closure,
            };

            *state_clone.borrow_mut() = Some(wasm_state);

            // Autonomous loop. The engine owns the frame logic (clock advance
            // is its own `StdClock`, no manual forwarding); this driver just
            // arms rAF / setTimeout per the engine's `NextFrame` verdict. The
            // `after_frame` hook — fired by the engine after each wake — does
            // the DOM side-effects that used to live in `FrameLoop::on_frame`
            // (file-pick resolution, textarea focus / caret positioning). It
            // holds a `Weak` into `state` so there's no reference cycle
            // (`state` → `app` → `after_frame` → `state`).
            let app = state_clone
                .borrow()
                .as_ref()
                .expect("wasm state just set")
                .app
                .clone();
            let state_weak: Weak<RefCell<Option<WasmState>>> = Rc::downgrade(&state_clone);
            let after_frame: Rc<dyn Fn(tur_engine::core::app::FrameOutcome)> =
                Rc::new(move |_outcome| {
                    let Some(state) = state_weak.upgrade() else {
                        return;
                    };
                    let mut guard = state.borrow_mut();
                    let Some(s) = guard.as_mut() else {
                        return;
                    };

                    // Drain pending __turHost.pickFile resolutions: build the
                    // `{ name, bytes<ArrayBuffer> }` (or null) and invoke the
                    // callback from here, where a `&mut Context` is available.
                    let pending_picks: Vec<PendingPick> =
                        FILE_PICK_RESULTS.with(|q| q.borrow_mut().drain(..).collect());
                    if !pending_picks.is_empty() {
                        s.app.with_boa_context(|ctx| {
                            use boa_engine::object::builtins::{AlignedVec, JsArrayBuffer};
                            use boa_engine::object::JsObject;
                            use boa_engine::{js_string, JsValue};
                            for (cb, picked) in pending_picks {
                                let arg = match picked {
                                    Some((name, bytes)) => {
                                        let o = JsObject::with_object_proto(ctx.intrinsics());
                                        let _ = o.create_data_property(
                                            js_string!("name"),
                                            JsValue::from(js_string!(name.as_str())),
                                            ctx,
                                        );
                                        if let Ok(ab) = JsArrayBuffer::from_byte_block(
                                            AlignedVec::from_iter(0, bytes),
                                            ctx,
                                        ) {
                                            let _ = o.create_data_property(
                                                js_string!("bytes"),
                                                JsValue::from(ab),
                                                ctx,
                                            );
                                        }
                                        o.into()
                                    }
                                    None => JsValue::null(),
                                };
                                if let Err(e) = cb
                                    .call(&boa_engine::JsValue::undefined(), &[arg], ctx)
                                {
                                    tracing::error!("pickFile callback error: {e}");
                                }
                            }
                        });
                    }

                    let is_editable = s.app.focused_is_editable();
                    if is_editable {
                        let _ = s.textarea.focus();
                        if let Some((x, y, _w, _h)) = s.app.focused_cursor_rect() {
                            let _ = s.textarea.style().set_property("left", &format!("{x}px"));
                            let _ = s.textarea.style().set_property("top", &format!("{y}px"));
                        }
                    }
                });
            app.set_after_frame_hook(Some(after_frame));
            // `start` registers the driver and runs frame 1 (which processes
            // the resize pushed above), then arms follow-up wake-ups per the
            // engine's verdict.
            app.start(WasmLoopDriver::new());

            let app = TurWasmApp {
                state: state_clone,
            };
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

        // Request a paint; `request_paint` re-arms the autonomous loop (via
        // the driver's `request_next(Vsync)`), so the bundle renders on the
        // next frame without any manual pump.
        state.app.request_paint();

        Ok(())
    }

    /// Evaluate `js_source` as an ES module (supports real
    /// `import { ... } from "builtin:tur/..."`, resolved by the engine's module
    /// loader), then start the frame loop. The replacement for
    /// [`load_and_run_js`](Self::load_and_run_js) for module-mode bundles
    /// (e.g. the self-hosted playground `impl.js`).
    #[wasm_bindgen(js_name = loadAndRunModule)]
    pub fn load_and_run_module(&mut self, js_source: &str) -> Result<(), JsValue> {
        let mut guard = self.state.borrow_mut();
        let state = guard
            .as_mut()
            .ok_or_else(|| JsValue::from_str("app not initialized"))?;
        state
            .app
            .load_module(js_source)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        state.app.request_paint();

        Ok(())
    }

    /// Return a host-side dev-tool handle. Methods on `TurDevTool` eval the
    /// in-engine `turDevTool` global (which itself delegates to
    /// `__tur._dev_tool_*`), returning JSON strings for the host to parse.
    pub fn dev_tool(&self) -> TurDevTool {
        TurDevTool {
            state: self.state.clone(),
        }
    }
}

/// Host-side dev-tool handle, exposed as `globalThis.turDevTool` by the
/// playground bootstrap. Methods return JSON strings (the data originates
/// inside the boa engine, a separate JS realm, so JSON is the simplest
/// cross-realm transport).
#[wasm_bindgen]
pub struct TurDevTool {
    state: Rc<RefCell<Option<WasmState>>>,
}

#[wasm_bindgen]
impl TurDevTool {
    /// JSON snapshot of the root node, or `""` if no tree is mounted.
    /// Shape: `{ id, name, label, props, layout:{relative,absolute,width,height,extra?}, queryKey?, children:[{id}, ...] }`.
    #[allow(non_snake_case)]
    pub fn elementTree(&self) -> String {
        let mut guard = self.state.borrow_mut();
        let Some(s) = guard.as_mut() else { return String::new() };
        s.app
            .eval_js("JSON.stringify(turDevTool.elementTree())")
            .unwrap_or_default()
    }

    /// JSON snapshot of a single node by id (full subtree metadata; children
    /// are returned as bare `{id}` handles). Returns `""` if not found.
    #[allow(non_snake_case)]
    pub fn getElement(&self, id: u32) -> String {
        let mut guard = self.state.borrow_mut();
        let Some(s) = guard.as_mut() else { return String::new() };
        s.app
            .eval_js(&format!("JSON.stringify(turDevTool.getElement({id}))"))
            .unwrap_or_default()
    }
}

/// `LoopDriver` for the wasm embedder, backed by `requestAnimationFrame`
/// (`Vsync`) and `setTimeout` (`After(d)`). The engine drives itself: each
/// autonomous frame runs in [`TurApp::wake`] (clock advance is the engine's
/// own `StdClock` — no manual time forwarding), the `after_frame` hook handles
/// DOM side-effects, and this driver just arms the next wake-up per the
/// engine's `NextFrame` verdict. When the engine reports `Idle`, the loop
/// stops entirely (zero rAF, zero renders) until [`TurApp::push_platform_event`]
/// re-arms it via `request_next(Vsync)`.
///
/// The two `Closure`s are long-lived and re-registered for every wake-up
/// (created once via `Rc::new_cyclic` with `Weak` back-refs) so a wake that
/// requests the next wake-up mid-invocation doesn't drop its own trampoline —
/// the same closure-lifetime fix the old `FrameLoop` needed.
struct WasmLoopDriver {
    /// Engine wake trampoline, set once via [`LoopDriver::set_wake`] at
    /// [`TurApp::start`].
    wake: RefCell<Option<Rc<dyn Fn()>>>,
    /// Pending rAF / setTimeout handle, if any. `None` ⇒ nothing pending
    /// (the loop is idle). Cleared by the trampoline when it fires.
    raf_id: Cell<Option<i32>>,
    timeout_id: Cell<Option<i32>>,
    raf_closure: Closure<dyn Fn()>,
    timeout_closure: Closure<dyn Fn()>,
}

impl WasmLoopDriver {
    fn new() -> Rc<Self> {
        Rc::<Self>::new_cyclic(|weak| {
            let weak_raf = weak.clone();
            let raf_closure = Closure::<dyn Fn()>::new(move || {
                if let Some(d) = weak_raf.upgrade() {
                    d.fire_raf();
                }
            });
            let weak_to = weak.clone();
            let timeout_closure = Closure::<dyn Fn()>::new(move || {
                if let Some(d) = weak_to.upgrade() {
                    d.fire_timeout();
                }
            });
            Self {
                wake: RefCell::new(None),
                raf_id: Cell::new(None),
                timeout_id: Cell::new(None),
                raf_closure,
                timeout_closure,
            }
        })
    }

    /// rAF trampoline entry: clear the handle, then fire the engine wake.
    fn fire_raf(&self) {
        self.raf_id.set(None);
        if let Some(wake) = self.wake.borrow().as_ref().cloned() {
            wake();
        }
    }

    /// setTimeout trampoline entry: clear the handle, then fire the wake
    /// (which re-arms via `request_next` for any further scheduling).
    fn fire_timeout(&self) {
        self.timeout_id.set(None);
        if let Some(wake) = self.wake.borrow().as_ref().cloned() {
            wake();
        }
    }

    /// Cancel any pending rAF / setTimeout so a fresh `request_next` starts
    /// from a clean slate (avoids double-firing when input re-arms an idle
    /// loop that had a timer outstanding).
    fn cancel_pending(&self) {
        if let Some(id) = self.raf_id.take() {
            if let Some(window) = web_sys::window() {
                let _ = window.cancel_animation_frame(id);
            }
        }
        if let Some(id) = self.timeout_id.take() {
            if let Some(window) = web_sys::window() {
                window.clear_timeout_with_handle(id);
            }
        }
    }
}

impl LoopDriver for WasmLoopDriver {
    fn set_wake(&self, wake: Rc<dyn Fn()>) {
        *self.wake.borrow_mut() = Some(wake);
    }

    fn request_next(&self, next: NextFrame) {
        self.cancel_pending();
        let Some(window) = web_sys::window() else {
            return;
        };
        match next {
            NextFrame::Idle => {}
            NextFrame::Vsync => {
                let id = window
                    .request_animation_frame(self.raf_closure.as_ref().unchecked_ref())
                    .unwrap_or(-1);
                if id >= 0 {
                    self.raf_id.set(Some(id));
                }
            }
            NextFrame::After(delay) => {
                let ms = delay.as_millis().min(i32::MAX as u128) as i32;
                let id = window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        self.timeout_closure.as_ref().unchecked_ref(),
                        ms.max(1),
                    )
                    .unwrap_or(0);
                self.timeout_id.set(Some(id));
            }
        }
    }
}

