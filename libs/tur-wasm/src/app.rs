use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tur_engine::TurApp;
use tur_engine::core::event::{AppEvent, AppGestureEvent, AppImeEvent};
use tur_engine::core::fonts::PresetFontLoader;
use tur_engine::core::keyboard::{AppKeyEvent, KeyEventType, Modifiers};
use tur_engine::renderer::vello::WebGlVelloRenderer;
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
    _context_closure: Closure<dyn Fn(web_sys::MouseEvent)>,
    _keydown_closure: Closure<dyn Fn(web_sys::KeyboardEvent)>,
    _keyup_closure: Closure<dyn Fn(web_sys::KeyboardEvent)>,
    _compositionstart_closure: Closure<dyn Fn(web_sys::CompositionEvent)>,
    _compositionupdate_closure: Closure<dyn Fn(web_sys::CompositionEvent)>,
    _compositionend_closure: Closure<dyn Fn(web_sys::CompositionEvent)>,
    _paste_closure: Closure<dyn Fn(web_sys::ClipboardEvent)>,
    _raf_closure: RefCell<Option<Closure<dyn Fn()>>>,
}

/// Embedder-side `CursorPlatform`: the engine pushes the resolved cursor here
/// during the frame loop, and we apply it to the host canvas.
struct WasmCursorPlatform {
    canvas: web_sys::HtmlCanvasElement,
}

impl tur_std::CursorPlatform for WasmCursorPlatform {
    fn set_cursor(&mut self, cursor: tur_shared::Cursor) {
        let _ = self.canvas.style().set_property("cursor", cursor.as_str());
    }
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

    // Clipboard write bridge — used by the engine's editable text Cmd+C /
    // Cmd+X handling (which extracts the selected text and pushes
    // AppEvent::ClipboardWrite). The wasm layer owns the actual browser
    // clipboard interaction. Fire-and-forget — the returned Promise is
    // discarded.
    let clipboard_write = NativeFunction::from_copy_closure(|_this, args, _ctx| {
        let text = args
            .get_or_undefined(0)
            .as_string()
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        if let Some(window) = web_sys::window() {
            let clipboard = window.navigator().clipboard();
            // Fire-and-forget — discard the returned Promise.
            let _ = clipboard.write_text(&text);
        }
        Ok(JsValue::undefined())
    });

    // Clipboard read bridge — `clipboardReadText(callback)`. The fn captures
    // the JS callback, kicks off a browser-side `navigator.clipboard.read_text()`
    // future, and on completion pushes (callback, text) into a wasm-side slot.
    // The frame loop drains that slot once per frame, invoking each callback
    // from within the boa context (the only place we have a `&mut Context`).
    // Resolves with an empty string if the browser denies the read.
    //
    // We can't return a Promise directly because resolving one from outside a
    // `&mut Context` is impossible — the callback-based API is equivalent and
    // avoids the borrow issue.
    let clipboard_read = NativeFunction::from_copy_closure(move |_this, args, _ctx| {
        use boa_engine::object::builtins::JsFunction;
        let Some(cb_obj) = args.get_or_undefined(0).as_object() else {
            return Ok(JsValue::undefined());
        };
        let Some(cb) = JsFunction::from_object(cb_obj.clone()) else {
            return Ok(JsValue::undefined());
        };
        wasm_bindgen_futures::spawn_local(async move {
            let text = match web_sys::window() {
                Some(window) => {
                    let promise = window.navigator().clipboard().read_text();
                    match wasm_bindgen_futures::JsFuture::from(promise).await {
                        Ok(v) => v.as_string().unwrap_or_default(),
                        Err(_) => String::new(),
                    }
                }
                None => String::new(),
            };
            // Stash (callback, text) on a thread-local queue. The frame
            // loop drains it next tick with a Context available.
            CLIPBOARD_READ_QUEUE.with(|q| {
                q.borrow_mut().push((cb, text));
            });
        });
        Ok(JsValue::undefined())
    });

    vec![
        ("transpileTsx", transpile, 1),
        ("tokenizeTsx", tokenize, 1),
        ("generateAst", generate_ast, 1),
        ("clipboardWriteText", clipboard_write, 1),
        ("clipboardReadText", clipboard_read, 1),
    ]
}

// ---------------------------------------------------------------------------
// `__tur.request()` — Promise-based HTTP client backed by reqwest-wasm.
//
// Lives in tur-wasm (reqwest-wasm is wasm-only); the engine exposes only the
// generic `register_tur_fn` hook that attaches this onto `globalThis.__tur`.
//
// Scheduling: the fn creates a pending `JsPromise`, spawns the reqwest future
// via `wasm_bindgen_futures::spawn_local`, and on completion pushes
// `(ResolvingFunctions, outcome)` into `HTTP_RESULTS`. The frame loop drains
// that queue inside `with_boa_context` (the only place a `&mut Context` is
// available) and resolves/rejects the promise — which enqueues a PromiseJob
// that runs on the next `flush`, so `.then`/`await` bodies fire in the same
// reactive pass as any `set()` they perform.
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum HttpBody {
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(Debug)]
enum HttpOutcome {
    Ok {
        status: u16,
        status_text: String,
        headers: Vec<(String, String)>,
        body: HttpBody,
    },
    Err(String),
}

/// A pending `__tur.request` result awaiting settlement during the frame loop.
type PendingHttp = (boa_engine::builtins::promise::ResolvingFunctions, HttpOutcome);

/// A pending `__turHost.pickFile` result: callback + (`None` if cancelled).
type PendingPick = (
    boa_engine::object::builtins::JsFunction,
    Option<(String, Vec<u8>)>,
);

/// The `change` handler closure type used by the hidden file-picker `<input>`.
type PickHandler = wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>;

async fn perform_request(
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Option<HttpBody>,
    want_bytes: bool,
    username: Option<String>,
    password: Option<String>,
) -> HttpOutcome {
    let result: Result<HttpOutcome, String> = async {
        let client = reqwest_wasm::Client::new();
        let m = reqwest_wasm::Method::from_bytes(method.as_bytes())
            .map_err(|e| format!("invalid method {method:?}: {e}"))?;
        let mut rb = client.request(m, &url);
        if let (Some(u), Some(p)) = (username.as_deref(), password.as_deref()) {
            rb = rb.basic_auth(u, Some(p));
        }
        for (k, v) in &headers {
            if let (Ok(name), Ok(val)) = (
                reqwest_wasm::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest_wasm::header::HeaderValue::from_str(v),
            ) {
                rb = rb.header(name, val);
            }
        }
        rb = match body {
            Some(HttpBody::Text(s)) => rb.body(s),
            Some(HttpBody::Bytes(b)) => rb.body(b),
            None => rb,
        };
        let resp = rb.send().await.map_err(|e| format!("{e}"))?;
        let status = resp.status().as_u16();
        let status_text = resp
            .status()
            .canonical_reason()
            .unwrap_or("")
            .to_string();
        let hdrs: Vec<(String, String)> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = if want_bytes {
            HttpBody::Bytes(resp.bytes().await.map_err(|e| format!("{e}"))?.to_vec())
        } else {
            HttpBody::Text(resp.text().await.map_err(|e| format!("{e}"))?)
        };
        Ok(HttpOutcome::Ok {
            status,
            status_text,
            headers: hdrs,
            body,
        })
    }
    .await;
    result.unwrap_or_else(HttpOutcome::Err)
}

fn js_opt_str(
    obj: &boa_engine::object::JsObject,
    key: &str,
    ctx: &mut boa_engine::Context,
) -> Option<String> {
    obj.get(boa_engine::js_string!(key), ctx)
        .ok()
        .and_then(|v| v.as_string().map(|s| s.to_std_string_escaped()))
}

fn build_net_fns() -> Vec<(&'static str, boa_engine::NativeFunction, usize)> {
    use boa_engine::native_function::NativeFunction;
    use boa_engine::object::builtins::{JsArrayBuffer, JsPromise};
    use boa_engine::property::PropertyKey;
    use boa_engine::{js_string, JsArgs, JsValue};

    // `request({ url, method?, headers?, body?, responseType?, username?, password? }) -> Promise`
    //
    // `body` accepts a string or an ArrayBuffer (from `pickFile`).
    // `responseType` is "text" (default; fills `bodyText`) or "bytes" (fills
    // `bodyBytes` as an ArrayBuffer). The resolved value is always an object:
    //   { ok: true, status, statusText, headers: {name:value}, bodyText?|bodyBytes? }
    // Errors reject with { message }.
    let request = NativeFunction::from_copy_closure(move |_this, args, ctx| {
        let (promise, resolvers) = JsPromise::new_pending(ctx);
        let opts = args.get_or_undefined(0);
        let Some(obj) = opts.as_object() else {
            let msg = JsValue::from(js_string!("request: options object required"));
            let _ = resolvers.reject.call(&JsValue::undefined(), &[msg], ctx);
            return Ok(promise.into());
        };

        let url = js_opt_str(&obj, "url", ctx).unwrap_or_default();
        let method = js_opt_str(&obj, "method", ctx).unwrap_or_else(|| "GET".to_string());
        let response_type =
            js_opt_str(&obj, "responseType", ctx).unwrap_or_else(|| "text".to_string());
        let username = js_opt_str(&obj, "username", ctx);
        let password = js_opt_str(&obj, "password", ctx);

        let mut headers: Vec<(String, String)> = Vec::new();
        if let Some(hobj) = obj
            .get(js_string!("headers"), ctx)
            .ok()
            .and_then(|v| v.as_object())
        {
            if let Ok(keys) = hobj.own_property_keys(ctx) {
                for key in keys {
                    let kstr = match &key {
                        PropertyKey::String(s) => s.to_std_string_escaped(),
                        PropertyKey::Index(i) => i.get().to_string(),
                        PropertyKey::Symbol(_) => continue,
                    };
                    if let Ok(v) = hobj.get(key, ctx) {
                        let vstr = v
                            .as_string()
                            .map(|s| s.to_std_string_escaped())
                            .unwrap_or_default();
                        headers.push((kstr, vstr));
                    }
                }
            }
        }

        let body: Option<HttpBody> = match obj.get(js_string!("body"), ctx) {
            Ok(v) => {
                if let Some(s) = v.as_string() {
                    Some(HttpBody::Text(s.to_std_string_escaped()))
                } else if let Some(o) = v.as_object() {
                    JsArrayBuffer::from_object(o.clone())
                        .ok()
                        .and_then(|ab| ab.to_vec())
                        .map(HttpBody::Bytes)
                } else {
                    None
                }
            }
            Err(_) => None,
        };

        let want_bytes = response_type == "bytes";

        wasm_bindgen_futures::spawn_local(async move {
            let outcome =
                perform_request(url, method, headers, body, want_bytes, username, password).await;
            HTTP_RESULTS.with(|q| q.borrow_mut().push((resolvers, outcome)));
        });

        Ok(promise.into())
    });

    vec![("request", request, 1)]
}

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
    /// Pending (callback, resolved text) pairs queued by `clipboardReadText`.
    /// Drained by `TurWasmApp::drain_clipboard_reads` from within the frame
    /// loop, where a `&mut Context` is available.
    static CLIPBOARD_READ_QUEUE: std::cell::RefCell<Vec<(boa_engine::object::builtins::JsFunction, String)>>
        = const { std::cell::RefCell::new(Vec::new()) };

    /// Pending `(ResolvingFunctions, HttpOutcome)` pairs queued by
    /// `__tur.request` once the reqwest future resolves. Drained in the frame
    /// loop, where a `&mut Context` is available to settle the promise.
    static HTTP_RESULTS: std::cell::RefCell<Vec<PendingHttp>>
        = const { std::cell::RefCell::new(Vec::new()) };

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

            let renderer = WebGlVelloRenderer::new(
                canvas.clone(),
                logical_width,
                logical_height,
                dpr,
            );

            let host_fns = build_host_service_fns();
            let file_fns = build_file_io_fns();
            let net_fns = build_net_fns();

            let host_exports: Vec<(String, boa_engine::NativeFunction, usize)> = host_fns
                .into_iter()
                .chain(file_fns)
                .map(|(n, f, l)| (n.to_string(), f, l))
                .collect();
            let net_exports: Vec<(String, boa_engine::NativeFunction, usize)> = net_fns
                .into_iter()
                .map(|(n, f, l)| (n.to_string(), f, l))
                .collect();

            let mut app = tur_engine::TurEngine::builder()
                .renderer(Box::new(renderer))
                .font_loader(Box::new(PresetFontLoader::new()))
                .plugin(
                    tur_std::TurStdPlugin::builder()
                        .cursor(WasmCursorPlatform {
                            canvas: canvas.clone(),
                        })
                        .build(),
                )
                .host_module("builtin:tur/host", host_exports)
                .host_module("builtin:tur/net", net_exports)
                .build()
                .map_err(|e| JsValue::from_str(&e.to_string()))?;

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
                        let button = tur_shared::MouseButton::from_dom(event.button() as u16);
                        // DOM `MouseEvent.timeStamp` is ms since epoch — used by
                        // the engine's gesture composer for multi-click
                        // (double/triple) classification.
                        let time_ms = event.time_stamp() as u64;
                        s.app.push_event(AppEvent::Gesture(
                            AppGestureEvent::PointerDown {
                                position: Offset::new(x, y),
                                button,
                                time_ms,
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
                        let button = tur_shared::MouseButton::from_dom(event.button() as u16);
                        s.app.push_event(AppEvent::Gesture(
                            AppGestureEvent::PointerUp {
                                position: Offset::new(x, y),
                                button,
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

            // Context menu (right-click) listener. We prevent the default
            // browser menu and forward the click position to the engine,
            // which dispatches a `ContextMenu` gesture to every element in
            // the hit-path.
            let context_state = state_clone.clone();
            let context_closure =
                Closure::<dyn Fn(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                    event.prevent_default();
                    let guard = context_state.borrow();
                    if let Some(s) = guard.as_ref() {
                        let rect = s._canvas.get_bounding_client_rect();
                        let x = event.client_x() as f64 - rect.left();
                        let y = event.client_y() as f64 - rect.top();
                        s.app.push_event(AppEvent::Gesture(
                            AppGestureEvent::ContextMenu {
                                position: Offset::new(x, y),
                            },
                        ));
                    }
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

            // Paste listener — when the user presses Cmd+V (or Ctrl+V) while
            // the hidden textarea is focused, the browser fires a `paste`
            // event with `clipboardData`. We forward the text to the engine
            // via AppEvent::ClipboardPaste, which the engine's
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
                        s.app.push_event(AppEvent::ClipboardPaste { text });
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
                _pointer_down_closure: pointer_down_closure,
                _pointer_up_closure: pointer_up_closure,
                _pointer_move_closure: pointer_move_closure,
                _wheel_closure: wheel_closure,
                _context_closure: context_closure,
                _keydown_closure: keydown_closure,
                _keyup_closure: keyup_closure,
                _compositionstart_closure: compositionstart_closure,
                _compositionupdate_closure: compositionupdate_closure,
                _compositionend_closure: compositionend_closure,
                _paste_closure: paste_closure,
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

        state.app.push_event(AppEvent::RequestDraw);
        if let Err(e) = state.app.spawn_loop_once(std::time::Duration::ZERO) {
            tracing::error!("load_and_run_module: initial spawn_loop_once error: {e}");
        }

        drop(guard);
        Self::start_frame_loop(&self.state);

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

impl TurWasmApp {
    fn start_frame_loop(state: &Rc<RefCell<Option<WasmState>>>) {
        let loop_state = state.clone();
        let raf_closure = Closure::<dyn Fn()>::new(move || {
            let mut guard = loop_state.borrow_mut();
            if let Some(s) = guard.as_mut() {
                if let Err(e) = s.app.spawn_loop_once(std::time::Duration::from_millis(16)) {
                    tracing::error!("frame loop spawn_loop_once error: {e}");
                }

                // Drain any pending clipboard-read resolutions. Each entry
                // is a (JS callback, text) pair queued by the
                // `__turHost.clipboardReadText(cb)` host fn when the
                // browser's `navigator.clipboard.read_text()` future
                // completed. We invoke the callback from here because a
                // `&mut Context` is available via `with_boa_context`.
                let pending: Vec<(boa_engine::object::builtins::JsFunction, String)> =
                    CLIPBOARD_READ_QUEUE.with(|q| q.borrow_mut().drain(..).collect());
                if !pending.is_empty() {
                    s.app.with_boa_context(|ctx| {
                        for (cb, text) in pending {
                            let text_val = boa_engine::JsValue::from(boa_engine::js_string!(text.as_str()));
                            if let Err(e) = cb.call(&boa_engine::JsValue::undefined(), &[text_val], ctx) {
                                tracing::error!("clipboardReadText callback error: {e}");
                            }
                        }
                    });
                }

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
                                    if let Ok(ab) =
                                        JsArrayBuffer::from_byte_block(
                                            AlignedVec::from_iter(0, bytes),
                                            ctx,
                                        )
                                    {
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
                            if let Err(e) =
                                cb.call(&boa_engine::JsValue::undefined(), &[arg], ctx)
                            {
                                tracing::error!("pickFile callback error: {e}");
                            }
                        }
                    });
                }

                // Drain pending __tur.request resolutions: settle each
                // pending promise (resolve/reject), which enqueues a
                // PromiseJob that runs on the next `flush`.
                let pending_http: Vec<PendingHttp> =
                    HTTP_RESULTS.with(|q| q.borrow_mut().drain(..).collect());
                if !pending_http.is_empty() {
                    s.app.with_boa_context(|ctx| {
                        use boa_engine::object::builtins::{AlignedVec, JsArrayBuffer};
                        use boa_engine::object::JsObject;
                        use boa_engine::{js_string, JsValue};
                        for (resolvers, outcome) in pending_http {
                            match outcome {
                                HttpOutcome::Ok {
                                    status,
                                    status_text,
                                    headers,
                                    body,
                                } => {
                                    let o = JsObject::with_object_proto(ctx.intrinsics());
                                    let _ = o.create_data_property(
                                        js_string!("ok"),
                                        JsValue::from(true),
                                        ctx,
                                    );
                                    let _ = o.create_data_property(
                                        js_string!("status"),
                                        JsValue::from(status as f64),
                                        ctx,
                                    );
                                    let _ = o.create_data_property(
                                        js_string!("statusText"),
                                        JsValue::from(js_string!(status_text.as_str())),
                                        ctx,
                                    );
                                    let hmap = JsObject::with_object_proto(ctx.intrinsics());
                                    for (k, v) in &headers {
                                        let _ = hmap.create_data_property(
                                            js_string!(k.as_str()),
                                            JsValue::from(js_string!(v.as_str())),
                                            ctx,
                                        );
                                    }
                                    let _ = o.create_data_property(
                                        js_string!("headers"),
                                        JsValue::from(hmap),
                                        ctx,
                                    );
                                    match body {
                                        HttpBody::Text(t) => {
                                            let _ = o.create_data_property(
                                                js_string!("bodyText"),
                                                JsValue::from(js_string!(t.as_str())),
                                                ctx,
                                            );
                                        }
                                        HttpBody::Bytes(b) => {
                                            if let Ok(ab) = JsArrayBuffer::from_byte_block(
                                                AlignedVec::from_iter(0, b),
                                                ctx,
                                            ) {
                                                let _ = o.create_data_property(
                                                    js_string!("bodyBytes"),
                                                    JsValue::from(ab),
                                                    ctx,
                                                );
                                            }
                                        }
                                    }
                                    if let Err(e) = resolvers.resolve.call(
                                        &boa_engine::JsValue::undefined(),
                                        &[o.into()],
                                        ctx,
                                    ) {
                                        tracing::error!("request resolve error: {e}");
                                    }
                                }
                                HttpOutcome::Err(msg) => {
                                    let e = JsObject::with_object_proto(ctx.intrinsics());
                                    let _ = e.create_data_property(
                                        js_string!("message"),
                                        JsValue::from(js_string!(msg.as_str())),
                                        ctx,
                                    );
                                    if let Err(e) = resolvers.reject.call(
                                        &boa_engine::JsValue::undefined(),
                                        &[e.into()],
                                        ctx,
                                    ) {
                                        tracing::error!("request reject error: {e}");
                                    }
                                }
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
