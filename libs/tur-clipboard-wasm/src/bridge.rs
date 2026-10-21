//! The worker → main clipboard bridge. The engine's pump (and therefore
//! every capability future, including this backend's) runs on a Web Worker
//! — `navigator.clipboard` is `Window`-only and does not exist on a
//! `DedicatedWorkerGlobalScope`, so the browser calls must hop to the main
//! thread. The boa era solved the same problem with a frame-loop drain
//! (`take_clipboard_write()` on the host bridge); this is the same hop
//! adapted to the tagged-message channel the worker scheme already rides
//! (`{ t: "…", … }` payloads over `postMessage`):
//!
//! - worker → main: `{ t: "tur-clip-write", text }` (fire-and-forget) and
//!   `{ t: "tur-clip-read", rid }` (a read request with a correlation id).
//! - main → worker: `{ t: "tur-clip-read-result", rid, text }` (the reply;
//!   an empty text covers denial/absence — the backend's documented
//!   degradation).
//!
//! The main side serves the requests from the additive `message` listener
//! `tur-wasm` installs on every spawned worker ([`serve_host_message`]);
//! the worker side routes replies from the executor's `onmessage`
//! dispatcher ([`route_worker_message`]) into the pending-read slab the
//! `read_text` futures await.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use js_sys::{Object, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::MessageEvent;

const TAG_WRITE: &str = "tur-clip-write";
const TAG_READ: &str = "tur-clip-read";
const TAG_READ_RESULT: &str = "tur-clip-read-result";

// ---------------------------------------------------------------------------
// Worker side — post requests, await replies
// ---------------------------------------------------------------------------

thread_local! {
    /// Pending `read_text` round trips (rid → slot). The reply's `message`
    /// event fills the slot and wakes the awaiting future; the reply ALSO
    /// hits the executor's onmessage, whose non-factory branch would repoll
    /// anyway — the stored waker makes the wake explicit and order-safe.
    static PENDING_READS: RefCell<HashMap<u64, ReadSlot>> = RefCell::new(HashMap::new());
    static NEXT_RID: Cell<u64> = const { Cell::new(1) };
}

struct ReadSlot {
    text: Option<String>,
    waker: Option<Waker>,
}

/// The `read_text` round trip: resolves when the main thread's reply fills
/// the slot (or immediately empty if the request could not be posted).
pub struct PendingRead {
    rid: u64,
}

impl Future for PendingRead {
    type Output = String;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<String> {
        PENDING_READS.with(|p| {
            let mut p = p.borrow_mut();
            match p.get_mut(&self.rid) {
                Some(slot) => {
                    if let Some(text) = slot.text.take() {
                        p.remove(&self.rid);
                        Poll::Ready(text)
                    } else {
                        slot.waker = Some(cx.waker().clone());
                        Poll::Pending
                    }
                }
                // No slot = the reply was consumed elsewhere or never tracked
                // (shouldn't happen) — degrade to the documented empty read.
                None => Poll::Ready(String::new()),
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

/// Post a write request to the main thread. Returns `false` when not on a
/// worker (the caller falls back to the direct `Window` call).
pub fn worker_post_write(text: &str) -> bool {
    if !on_worker() {
        return false;
    }
    let payload = Object::new();
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("t"),
        &JsValue::from_str(TAG_WRITE),
    );
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("text"),
        &JsValue::from_str(text),
    );
    post_to_main(&payload)
}

/// Post a read request to the main thread. Returns `false` when not on a
/// worker (the caller falls back to the direct `Window` call).
pub fn worker_post_read(rid: u64) -> bool {
    if !on_worker() {
        return false;
    }
    let payload = Object::new();
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("t"),
        &JsValue::from_str(TAG_READ),
    );
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("rid"),
        &JsValue::from_f64(rid as f64),
    );
    post_to_main(&payload)
}

/// Mint the next read correlation id (worker-local).
fn next_rid() -> u64 {
    NEXT_RID.with(|n| {
        let rid = n.get();
        n.set(rid.wrapping_add(1));
        rid
    })
}

/// Begin a read round trip from the worker: register the pending slot, post
/// the request. `None` when not on a worker (the caller falls back to the
/// direct `Window` call).
pub fn worker_begin_read() -> Option<PendingRead> {
    if !on_worker() {
        return None;
    }
    let rid = next_rid();
    PENDING_READS.with(|p| {
        p.borrow_mut().insert(
            rid,
            ReadSlot {
                text: None,
                waker: None,
            },
        );
    });
    if !worker_post_read(rid) {
        PENDING_READS.with(|p| {
            p.borrow_mut().remove(&rid);
        });
        return None;
    }
    Some(PendingRead { rid })
}

/// The worker executor's `onmessage` hook: route a clipboard read reply into
/// its pending slot and wake the awaiting future. Returns `false` for
/// non-clipboard messages (the caller falls through to its own dispatch).
pub fn route_worker_message(data: &JsValue) -> bool {
    let tag = Reflect::get(data, &JsValue::from_str("t"))
        .ok()
        .and_then(|t| t.as_string());
    if tag.as_deref() != Some(TAG_READ_RESULT) {
        return false;
    }
    let rid = Reflect::get(data, &JsValue::from_str("rid"))
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as u64;
    let text = Reflect::get(data, &JsValue::from_str("text"))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();
    PENDING_READS.with(|p| {
        if let Some(slot) = p.borrow_mut().get_mut(&rid) {
            slot.text = Some(text);
            if let Some(w) = slot.waker.take() {
                w.wake();
            }
        }
    });
    true
}

// ---------------------------------------------------------------------------
// Main side — serve the requests (navigator.clipboard is Window-only)
// ---------------------------------------------------------------------------

/// The main thread's `message` listener body (installed per spawned worker by
/// tur-wasm): serve clipboard write/read requests from the worker. Returns
/// `false` for non-clipboard messages so the caller's other listeners chain.
pub fn serve_host_message(ev: &MessageEvent, worker: &web_sys::Worker) -> bool {
    let Some(window) = web_sys::window() else {
        return false; // not the main thread — not the serving side
    };
    let data = ev.data();
    let tag = Reflect::get(&data, &JsValue::from_str("t"))
        .ok()
        .and_then(|t| t.as_string());
    match tag.as_deref() {
        Some(TAG_WRITE) => {
            let text = Reflect::get(&data, &JsValue::from_str("text"))
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            // Fire-and-forget matches the boa-era host bridge — but the
            // promise's rejection is LOGGED now (a rejected writeText =
            // "copy silently does nothing", the bug shape that hid the
            // worker/thread mismatch for this whole feature).
            let promise = window.navigator().clipboard().write_text(&text);
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = wasm_bindgen_futures::JsFuture::from(promise).await {
                    web_sys::console::warn_1(&JsValue::from_str(&format!(
                        "tur clipboard: writeText rejected: {e:?}"
                    )));
                }
            });
            true
        }
        Some(TAG_READ) => {
            let rid = Reflect::get(&data, &JsValue::from_str("rid"))
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let promise = window.navigator().clipboard().read_text();
            let worker = worker.clone();
            wasm_bindgen_futures::spawn_local(async move {
                // A rejected read (permission denied, unsupported browser)
                // resolves with the documented empty-string degradation.
                let text = match wasm_bindgen_futures::JsFuture::from(promise).await {
                    Ok(v) => v.as_string().unwrap_or_default(),
                    Err(_) => String::new(),
                };
                reply_read(&worker, rid, &text);
            });
            true
        }
        _ => false,
    }
}

fn reply_read(worker: &web_sys::Worker, rid: f64, text: &str) {
    let payload = Object::new();
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("t"),
        &JsValue::from_str(TAG_READ_RESULT),
    );
    let _ = Reflect::set(&payload, &JsValue::from_str("rid"), &JsValue::from_f64(rid));
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("text"),
        &JsValue::from_str(text),
    );
    let _ = worker.post_message(&payload);
}
