//! File IO host bridges (browser-only): `pickFile(callback)` opens the native
//! file picker; `saveFile(name, bytes)` triggers a browser download.
//!
//! Bytes round-trip through boa ArrayBuffers; the actual File/Blob live in the
//! browser heap, so we copy through `Vec<u8>`.
//!
//! `pickFile` resolves the browser File bytes asynchronously (outside the
//! engine frame loop), so the callback + picked bytes are queued in
//! [`FILE_PICK_RESULTS`] and drained on the next frame by
//! [`resolve_pending_picks`] — which the wasm embedder calls from its
//! `after_frame` hook (where a `&mut Context` is available to build the
//! ArrayBuffer and invoke the callback).

use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::{JsArrayBuffer, JsFunction};
use boa_engine::Context;
use boa_engine::JsArgs;
use boa_engine::JsValue;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

/// A pending `pickFile` result: callback + (`None` if cancelled).
pub(crate) type PendingPick = (JsFunction, Option<(String, Vec<u8>)>);

/// The `change` handler closure type used by the hidden file-picker `<input>`.
type PickHandler = wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>;

thread_local! {
    /// Pending `(callback, picked-file)` pairs queued by `pickFile` once the
    /// browser File bytes are read. `None` = picker cancelled.
    static FILE_PICK_RESULTS: std::cell::RefCell<Vec<PendingPick>>
        = const { std::cell::RefCell::new(Vec::new()) };

    /// Keeps the per-pick `change` closures alive for the lifetime of the
    /// hidden `<input type=file>` (otherwise they'd be dropped before the user
    /// selects a file). Accumulates; entries outlive their use but the leak is
    /// negligible for a playground.
    static PICK_CLOSURES: std::cell::RefCell<Vec<PickHandler>>
        = const { std::cell::RefCell::new(Vec::new()) };
}

/// Build the file-IO host functions: `pickFile` + `saveFile`.
pub fn build_file_io_fns() -> Vec<(&'static str, boa_engine::NativeFunction, usize)> {
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
            if let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence(&parts)
                && let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                    if let Some(document) = document
                        && let Ok(a_el) = document.create_element("a")
                            && let Ok(a) = a_el.dyn_into::<web_sys::HtmlAnchorElement>() {
                                a.set_href(&url);
                                a.set_download(&name);
                                if let Some(body) = document.body() {
                                    let _ = body.append_child(&a);
                                    a.click();
                                    let _ = body.remove_child(&a);
                                }
                            }
                    let _ = web_sys::Url::revoke_object_url(&url);
                }
        }
        Ok(JsValue::undefined())
    });

    vec![
        ("pickFile", pick_file, 1),
        ("saveFile", save_file, 2),
    ]
}

/// Drain pending `pickFile` results and invoke each callback with
/// `{ name, bytes<ArrayBuffer> }` (or `null` if cancelled). Called by the
/// wasm embedder from its `after_frame` hook, where a `&mut Context` is
/// available. No-op when no picks are pending.
pub fn resolve_pending_picks(ctx: &mut Context) {
    use boa_engine::object::builtins::AlignedVec;
    use boa_engine::object::JsObject;
    use boa_engine::{js_string, JsValue};

    let pending: Vec<PendingPick> = FILE_PICK_RESULTS.with(|q| q.borrow_mut().drain(..).collect());
    if pending.is_empty() {
        return;
    }
    for (cb, picked) in pending {
        let arg = match picked {
            Some((name, bytes)) => {
                let o = JsObject::with_object_proto(ctx.intrinsics());
                let _ = o.create_data_property(
                    js_string!("name"),
                    JsValue::from(js_string!(name.as_str())),
                    ctx,
                );
                if let Ok(ab) =
                    JsArrayBuffer::from_byte_block(AlignedVec::from_iter(0, bytes), ctx)
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
        if let Err(e) = cb.call(&JsValue::undefined(), &[arg], ctx) {
            tracing::error!("pickFile callback error: {e}");
        }
    }
}
