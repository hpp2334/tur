//! `FilePickerBackend` impl backed by the browser DOM.
//!
//! `pick` opens a hidden `<input type="file">` and resolves the engine future
//! with the picked files' bytes once the `change` event fires. `save` builds a
//! `Blob` + temporary `<a download>` and triggers a browser download.
//!
//! The DOM `change` closure (which captures the JS Promise resolver) must
//! outlive the `pick` call — it's stashed in [`PICK_CLOSURES`] and lives until
//! the picker resolves. This is a small, bounded leak (one closure per pick,
//! roughly the size of the captured resolver handles); see the note on
//! [`PICK_CLOSURES`]. Cancel-detection is not wired: if the user dismisses the
//! dialog the `change` event never fires, so the promise stays pending
//! (matching the prior `tur-ext/demo-helper` behavior). A future revision can
//! listen for `window.focus` to resolve with an empty array on cancel.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;

use js_sys::Array as JsArray;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use tur_filepicker_capability::{FilePickerBackend, PickedFile};

/// The `change` handler closure type used by the hidden file-picker `<input>`.
type PickHandler = Closure<dyn FnMut(web_sys::Event)>;

// Keeps the per-pick DOM `change` closures alive until the picker resolves
// (otherwise the browser would collect the handler before the user selects a
// file). Entries outlive their pick — a bounded, negligible leak inherited
// from the prior playground implementation. A future revision can tie the
// closure's lifetime to the resolving future.
thread_local! {
    static PICK_CLOSURES: RefCell<Vec<PickHandler>> = const { RefCell::new(Vec::new()) };
}

/// Browser file-picker backend.
#[derive(Default)]
pub struct WasmFilePicker;

impl FilePickerBackend for WasmFilePicker {
    fn pick(
        &self,
        opts: tur_filepicker_capability::PickOptions,
    ) -> Pin<Box<dyn Future<Output = Vec<PickedFile>>>> {
        Box::pin(async move {
            let Some(window) = web_sys::window() else {
                return Vec::new();
            };
            let Some(document) = window.document() else {
                return Vec::new();
            };
            let Ok(input_el) = document.create_element("input") else {
                return Vec::new();
            };
            let Ok(input) = input_el.dyn_into::<web_sys::HtmlInputElement>() else {
                return Vec::new();
            };
            input.set_type("file");
            if opts.multiple {
                input.set_multiple(true);
            }
            if !opts.accept.is_empty() {
                input.set_accept(&opts.accept.join(","));
            }

            // Build a JS Promise that resolves with the picked `File[]` once
            // the input fires `change`. The actual byte-reading happens in the
            // awaiting future below (where `wasm_bindgen_futures` can drive it).
            let input_for_init = input.clone();
            let promise = js_sys::Promise::new(&mut move |resolve, _reject| {
                let input_for_handler = input_for_init.clone();
                let on_change = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev| {
                    let files_arr = JsArray::new();
                    if let Some(fl) = input_for_handler.files() {
                        for i in 0..fl.length() {
                            if let Some(file) = fl.get(i) {
                                files_arr.push(&file);
                            }
                        }
                    }
                    let _ = resolve.call1(&wasm_bindgen::JsValue::undefined(), &files_arr);
                });
                input_for_init.set_onchange(Some(on_change.as_ref().unchecked_ref()));
                PICK_CLOSURES.with(|c| c.borrow_mut().push(on_change));
                input_for_init.click();
            });

            let files_val = match JsFuture::from(promise).await {
                Ok(v) => v,
                Err(_) => return Vec::new(),
            };
            let files_arr = JsArray::from(&files_val);

            let mut picked: Vec<PickedFile> = Vec::new();
            for i in 0..files_arr.length() {
                let file = web_sys::File::from(files_arr.get(i));
                let name = file.name();
                let mime = file.type_();
                let bytes = match JsFuture::from(file.array_buffer()).await {
                    Ok(buf) => {
                        let arr = js_sys::Uint8Array::new(&buf);
                        let mut v = vec![0u8; arr.length() as usize];
                        arr.copy_to(&mut v);
                        v
                    }
                    Err(_) => continue,
                };
                picked.push(PickedFile {
                    name,
                    bytes,
                    mime_type: if mime.is_empty() { None } else { Some(mime) },
                });
            }
            picked
        })
    }

    fn save(
        &self,
        name: String,
        bytes: Vec<u8>,
        _opts: tur_filepicker_capability::SaveOptions,
    ) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            let Some(window) = web_sys::window() else {
                return;
            };
            let document = window.document();
            let arr = js_sys::Uint8Array::from(&bytes[..]);
            let parts = JsArray::new();
            parts.push(&arr);
            if let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence(&parts)
                && let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob)
            {
                if let Some(document) = document
                    && let Ok(a_el) = document.create_element("a")
                    && let Ok(a) = a_el.dyn_into::<web_sys::HtmlAnchorElement>()
                {
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
        })
    }
}
