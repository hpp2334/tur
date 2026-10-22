//! The worker → main filepicker bridge. The engine's pump (and therefore
//! every capability future, including this backend's) runs on a Web Worker
//! — `window` / `document` are `Window`-only and do not exist on a
//! `DedicatedWorkerGlobalScope`, so the DOM calls (the hidden
//! `<input type="file">`, the Blob + `<a download>` save) must hop to the
//! main thread. The clipboard bridge (`tur-clipboard-wasm`) solved the
//! identical realm problem; this is the same hop over the tagged-message
//! channel the worker scheme already rides (`{ t: "…", … }` payloads over
//! `postMessage`):
//!
//! - worker → main: `{ t: "tur-file-save", name, bytes }` (fire-and-forget
//!   — the download side-effect) and `{ t: "tur-file-pick", rid, multiple,
//!   accept }` (a pick request with a correlation id).
//! - main → worker: `{ t: "tur-file-pick-result", rid, files }` (the reply;
//!   `files` is an array of `{ name, mime, bytes }` records — an empty
//!   array covers cancel/denial, the backend's documented degradation).
//!
//! The main side serves the requests from the additive `message` listener
//! `tur-wasm` installs on every spawned worker ([`serve_host_message`]);
//! the worker side routes replies from the executor's `onmessage`
//! dispatcher ([`route_worker_message`]) into the pending-pick slab the
//! pick futures await.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use js_sys::{Object, Reflect};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::MessageEvent;

use tur_filepicker_capability::PickedFile;

const TAG_SAVE: &str = "tur-file-save";
const TAG_PICK: &str = "tur-file-pick";
const TAG_PICK_RESULT: &str = "tur-file-pick-result";

// ---------------------------------------------------------------------------
// Worker side — post requests, await replies
// ---------------------------------------------------------------------------

thread_local! {
    /// Pending `pick` round trips (rid → slot). The reply's `message` event
    /// fills the slot and wakes the awaiting future; the reply ALSO hits the
    /// executor's onmessage, whose non-factory branch would repoll anyway —
    /// the stored waker makes the wake explicit and order-safe.
    static PENDING_PICKS: RefCell<HashMap<u64, PickSlot>> = RefCell::new(HashMap::new());
    static NEXT_RID: Cell<u64> = const { Cell::new(1) };
}

struct PickSlot {
    files: Option<Vec<PickedFile>>,
    waker: Option<Waker>,
}

/// The `pick` round trip: resolves when the main thread's reply fills the
/// slot (or immediately empty if the request could not be posted).
pub struct PendingPick {
    rid: u64,
}

impl Future for PendingPick {
    type Output = Vec<PickedFile>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Vec<PickedFile>> {
        PENDING_PICKS.with(|p| {
            let mut p = p.borrow_mut();
            match p.get_mut(&self.rid) {
                Some(slot) => {
                    if let Some(files) = slot.files.take() {
                        p.remove(&self.rid);
                        Poll::Ready(files)
                    } else {
                        slot.waker = Some(cx.waker().clone());
                        Poll::Pending
                    }
                }
                // No slot = the reply was consumed elsewhere or never tracked
                // (shouldn't happen) — degrade to the documented empty pick.
                None => Poll::Ready(Vec::new()),
            }
        })
    }
}

/// Am I running on a Web Worker? (The engine pump's home; `web_sys::window()`
/// is `None` there.)
fn on_worker() -> bool {
    js_sys::global()
        .dyn_ref::<web_sys::DedicatedWorkerGlobalScope>()
        .is_some()
}

fn post_to_main(payload: &Object) -> bool {
    js_sys::global()
        .dyn_ref::<web_sys::DedicatedWorkerGlobalScope>()
        .map(|scope| scope.post_message(payload).is_ok())
        .unwrap_or(false)
}

/// Post a save request to the main thread. Returns `false` when not on a
/// worker (the caller falls back to the direct `Window` calls).
pub fn worker_post_save(name: &str, bytes: &[u8]) -> bool {
    if !on_worker() {
        return false;
    }
    let payload = Object::new();
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("t"),
        &JsValue::from_str(TAG_SAVE),
    );
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("name"),
        &JsValue::from_str(name),
    );
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("bytes"),
        &js_sys::Uint8Array::from(bytes),
    );
    post_to_main(&payload)
}

/// Mint the next pick correlation id (worker-local).
fn next_rid() -> u64 {
    NEXT_RID.with(|n| {
        let rid = n.get();
        n.set(rid.wrapping_add(1));
        rid
    })
}

/// Begin a pick round trip from the worker: register the pending slot, post
/// the request. `None` when not on a worker (the caller falls back to the
/// direct `Window` calls).
pub fn worker_begin_pick(multiple: bool, accept: &[String]) -> Option<PendingPick> {
    if !on_worker() {
        return None;
    }
    let rid = next_rid();
    PENDING_PICKS.with(|p| {
        p.borrow_mut().insert(
            rid,
            PickSlot {
                files: None,
                waker: None,
            },
        );
    });
    let payload = Object::new();
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("t"),
        &JsValue::from_str(TAG_PICK),
    );
    let _ = Reflect::set(&payload, &JsValue::from_str("rid"), &JsValue::from_f64(rid as f64));
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("multiple"),
        &JsValue::from_bool(multiple),
    );
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("accept"),
        &JsValue::from_str(&accept.join(",")),
    );
    if !post_to_main(&payload) {
        PENDING_PICKS.with(|p| {
            p.borrow_mut().remove(&rid);
        });
        return None;
    }
    Some(PendingPick { rid })
}

/// The worker executor's `onmessage` hook: route a filepicker pick reply
/// into its pending slot and wake the awaiting future. Returns `false` for
/// non-filepicker messages (the caller falls through to its own dispatch).
pub fn route_worker_message(data: &JsValue) -> bool {
    let tag = Reflect::get(data, &JsValue::from_str("t"))
        .ok()
        .and_then(|t| t.as_string());
    if tag.as_deref() != Some(TAG_PICK_RESULT) {
        return false;
    }
    let rid = Reflect::get(data, &JsValue::from_str("rid"))
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as u64;
    let mut picked: Vec<PickedFile> = Vec::new();
    if let Ok(files_val) = Reflect::get(data, &JsValue::from_str("files")) {
        let files = js_sys::Array::from(&files_val);
        for i in 0..files.length() {
            let entry = Object::from(files.get(i));
            let name = Reflect::get(&entry, &JsValue::from_str("name"))
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let mime = Reflect::get(&entry, &JsValue::from_str("mime"))
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let bytes = Reflect::get(&entry, &JsValue::from_str("bytes"))
                .ok()
                .and_then(|v| v.dyn_into::<js_sys::Uint8Array>().ok())
                .map(|arr| {
                    let mut v = vec![0u8; arr.length() as usize];
                    arr.copy_to(&mut v);
                    v
                })
                .unwrap_or_default();
            picked.push(PickedFile {
                name,
                bytes,
                mime_type: if mime.is_empty() { None } else { Some(mime) },
            });
        }
    }
    PENDING_PICKS.with(|p| {
        if let Some(slot) = p.borrow_mut().get_mut(&rid) {
            slot.files = Some(picked);
            if let Some(w) = slot.waker.take() {
                w.wake();
            }
        }
    });
    true
}

// ---------------------------------------------------------------------------
// Main side — serve the requests (window / document are Window-only)
// ---------------------------------------------------------------------------

thread_local! {
    /// The main side's per-pick DOM `change` closures (which capture the JS
    /// Promise resolver) — they must outlive the pick. The same bounded,
    /// negligible leak the direct backend had; see the backend's note.
    static MAIN_PICK_CLOSURES: RefCell<Vec<PickHandler>> = RefCell::new(Vec::new());
}

type PickHandler = Closure<dyn FnMut(web_sys::Event)>;

/// The main thread's `message` listener body (installed per spawned worker by
/// tur-wasm): serve file save/pick requests from the worker. Returns `false`
/// for non-filepicker messages so the caller's other listeners chain.
pub fn serve_host_message(ev: &MessageEvent, worker: &web_sys::Worker) -> bool {
    let Some(window) = web_sys::window() else {
        return false; // not the main thread — not the serving side
    };
    let data = ev.data();
    let tag = Reflect::get(&data, &JsValue::from_str("t"))
        .ok()
        .and_then(|t| t.as_string());
    match tag.as_deref() {
        Some(TAG_SAVE) => {
            let name = Reflect::get(&data, &JsValue::from_str("name"))
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let bytes = Reflect::get(&data, &JsValue::from_str("bytes"))
                .ok()
                .and_then(|v| v.dyn_into::<js_sys::Uint8Array>().ok())
                .map(|arr| {
                    let mut v = vec![0u8; arr.length() as usize];
                    arr.copy_to(&mut v);
                    v
                })
                .unwrap_or_default();
            perform_download(&window, &name, &bytes);
            true
        }
        Some(TAG_PICK) => {
            let rid = Reflect::get(&data, &JsValue::from_str("rid"))
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let multiple = Reflect::get(&data, &JsValue::from_str("multiple"))
                .ok()
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let accept = Reflect::get(&data, &JsValue::from_str("accept"))
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            serve_pick(&window, worker, rid, multiple, &accept);
            true
        }
        _ => false,
    }
}

/// Build the Blob + temporary `<a download>` and trigger the browser's
/// download — the backend's save DOM dance, run on the MAIN thread. Every
/// failure shape warns (a silent no-op here is the bug shape that hid the
/// worker/thread mismatch for the whole download journey).
fn perform_download(window: &web_sys::Window, name: &str, bytes: &[u8]) {
    let arr = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&arr);
    let blob = match web_sys::Blob::new_with_u8_array_sequence(&parts) {
        Ok(blob) => blob,
        Err(e) => {
            web_sys::console::warn_1(&JsValue::from_str(&format!(
                "tur filepicker: save Blob build failed: {e:?}"
            )));
            return;
        }
    };
    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(url) => url,
        Err(e) => {
            web_sys::console::warn_1(&JsValue::from_str(&format!(
                "tur filepicker: createObjectURL failed: {e:?}"
            )));
            return;
        }
    };
    if let Some(document) = window.document() {
        match document.create_element("a") {
            Ok(a_el) => match a_el.dyn_into::<web_sys::HtmlAnchorElement>() {
                Ok(a) => {
                    a.set_href(&url);
                    a.set_download(name);
                    if let Some(body) = document.body() {
                        let _ = body.append_child(&a);
                        a.click();
                        let _ = body.remove_child(&a);
                    }
                }
                Err(e) => {
                    web_sys::console::warn_1(&JsValue::from_str(&format!(
                        "tur filepicker: anchor cast failed: {e:?}"
                    )));
                }
            },
            Err(e) => {
                web_sys::console::warn_1(&JsValue::from_str(&format!(
                    "tur filepicker: anchor create failed: {e:?}"
                )));
            }
        }
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

/// Open the hidden `<input type="file">` on the main thread and reply with
/// the picked files' bytes once `change` fires (an empty array covers
/// cancel/denial — the backend's documented degradation).
fn serve_pick(window: &web_sys::Window, worker: &web_sys::Worker, rid: f64, multiple: bool, accept: &str) {
    let Some(document) = window.document() else {
        reply_pick(worker, rid, &js_sys::Array::new());
        return;
    };
    let Ok(input_el) = document.create_element("input") else {
        reply_pick(worker, rid, &js_sys::Array::new());
        return;
    };
    let Ok(input) = input_el.dyn_into::<web_sys::HtmlInputElement>() else {
        reply_pick(worker, rid, &js_sys::Array::new());
        return;
    };
    input.set_type("file");
    if multiple {
        input.set_multiple(true);
    }
    if !accept.is_empty() {
        input.set_accept(accept);
    }

    // A Promise that resolves with the picked `File[]` once the input fires
    // `change`; the byte-reading happens in the awaiting spawn below.
    let input_for_init = input.clone();
    let promise = js_sys::Promise::new(&mut move |resolve, _reject| {
        let input_for_handler = input_for_init.clone();
        let on_change = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev| {
            let files_arr = js_sys::Array::new();
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
        MAIN_PICK_CLOSURES.with(|c| c.borrow_mut().push(on_change));
        input_for_init.click();
    });

    let worker = worker.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let files_val = match wasm_bindgen_futures::JsFuture::from(promise).await {
            Ok(v) => v,
            Err(_) => {
                reply_pick(&worker, rid, &js_sys::Array::new());
                return;
            }
        };
        let files = js_sys::Array::from(&files_val);
        let out = js_sys::Array::new();
        for i in 0..files.length() {
            let file = web_sys::File::from(files.get(i));
            let name = file.name();
            let mime = file.type_();
            let bytes = match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
                Ok(buf) => js_sys::Uint8Array::new(&buf),
                Err(_) => continue,
            };
            let entry = Object::new();
            let _ = Reflect::set(&entry, &JsValue::from_str("name"), &JsValue::from_str(&name));
            let _ = Reflect::set(&entry, &JsValue::from_str("mime"), &JsValue::from_str(&mime));
            let _ = Reflect::set(&entry, &JsValue::from_str("bytes"), &bytes);
            out.push(&entry);
        }
        reply_pick(&worker, rid, &out);
    });
}

fn reply_pick(worker: &web_sys::Worker, rid: f64, files: &js_sys::Array) {
    let payload = Object::new();
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("t"),
        &JsValue::from_str(TAG_PICK_RESULT),
    );
    let _ = Reflect::set(&payload, &JsValue::from_str("rid"), &JsValue::from_f64(rid));
    let _ = Reflect::set(&payload, &JsValue::from_str("files"), files);
    let _ = worker.post_message(&payload);
}
