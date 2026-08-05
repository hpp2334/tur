//! In-tree Web Worker spawner + cooperative mini-executor (replaces
//! `wasm_thread`).
//!
//! ## Why not `wasm_thread`?
//!
//! `wasm_thread` 0.3.3 closes the worker (`close()`) the moment its entry
//! closure returns, and does not expose the underlying `web_sys::Worker`
//! handle (`JoinHandle::thread()` is `unimplemented!()`). Both are blockers
//! for an async worker:
//!
//! - To keep the worker alive without a sync `block_on`, the entry must
//!   return (after installing wake handlers) and the worker must NOT be
//!   closed — impossible with `wasm_thread`'s `close()`-on-return.
//! - To wake an idle Web Worker's JS event loop cross-thread without a
//!   sync `Atomics.wait` (which freezes the loop), main must
//!   `worker.postMessage(0)` — needing the `Worker` handle.
//!
//! So we spawn the worker ourselves: same shared-memory wasm module/memory
//! (the `Arc`-based `futures::channel::mpsc` channels still work
//! cross-thread), a custom bootstrap script that does NOT call `close()`,
//! and the `Worker` handle is kept by the scheduler for `postMessage` wake.
//!
//! ## Mini-executor
//!
//! `loop_fut` is polled cooperatively. `schedule_repoll()` arms a
//! `setTimeout(0)` that polls once; it is triggered by:
//! - **Cross-thread wake** — main posts `0`; the worker's `onmessage`
//!   handler calls `schedule_repoll()`.
//! - **Same-thread wake** — the waker registered by `loop_fut`'s internal
//!   `worker_rx.next().await` (or any sub-future). The waker is a
//!   `NoopWaker` that only acts when fired *on the worker thread*; a
//!   cross-thread fire (mpsc send from main) is a no-op (postMessage
//!   handles it).
//!
//! This never calls `Atomics.wait`, so the worker's JS event loop is never
//! frozen: `setTimeout` timers (e.g. `sleep`), promise reactions, and
//! `spawn_local`'d I/O futures all run freely between polls.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Wake, Waker};
use std::thread::ThreadId;

use js_sys::Reflect;
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use web_sys::MessageEvent;

use tur_engine::core::scheduler::WorkerScheduler;

use crate::scheduler::WasmWorkerScheduler;

/// The engine's `worker_loop` factory: runs on the worker thread,
/// constructs the backend (`!Send` types built there), returns the
/// worker's main future. `Send + 'static` so it can be boxed on main,
/// posted as a raw pointer, and reconstituted on the worker (valid because
/// the wasm linear memory is shared across threads).
pub(crate) type LoopFactory = Box<
    dyn FnOnce(Rc<dyn WorkerScheduler>) -> Pin<Box<dyn Future<Output = ()> + 'static>>
        + Send
        + 'static,
>;

/// Wrapper around [`LoopFactory`] so it can be boxed + passed as a raw
/// pointer (the engine hands us an already-boxed `dyn FnOnce`).
struct WorkerEntry(LoopFactory);

/// Per-worker executor state. Held in a `thread_local` (`Rc`-shared between
/// the poll closure, the wake handler, and the waker). `!Send` — lives only
/// on the worker thread.
struct ExecutorState {
    /// The `loop_fut`. `None` once it has completed (the worker then idles
    /// until `close()`, which never comes — the worker outlives the loop).
    task: RefCell<Option<Pin<Box<dyn Future<Output = ()> + 'static>>>>,
    /// `setTimeout(0)` repoll guard — at most one repoll is armed at a time.
    scheduled: Cell<bool>,
    /// The reused `setTimeout` poll callback (a leaked `Closure`'s JS ref).
    poll_fn: RefCell<Option<js_sys::Function>>,
    /// Set when `loop_fut` returned `Ready` — stops further polls.
    done: Cell<bool>,
}

thread_local! {
    static EXEC: RefCell<Option<Rc<ExecutorState>>> = const { RefCell::new(None) };
    static WORKER_TID: Cell<Option<ThreadId>> = const { Cell::new(None) };
}

// ---------------------------------------------------------------------------
// Main-side spawn
// ---------------------------------------------------------------------------

/// Spawn a shared-memory Web Worker, post the init payload, return the
/// `Worker` handle. The worker imports the same wasm_bindgen shim main
/// uses, inits the shared module+memory, then calls [`tur_worker_main`]
/// with the boxed-factory pointer.
pub(crate) fn spawn(factory: LoopFactory) -> web_sys::Worker {
    // Locate the wasm_bindgen shim URL (the same `.js` main uses) by
    // probing the stack trace. The worker `import`s init +
    // `tur_worker_main` from it.
    let shim_url = js_sys::eval(include_str!("js/script_path.js"))
        .ok()
        .and_then(|v| v.as_string())
        .expect("tur worker: failed to locate wasm_bindgen shim URL");

    // Build the bootstrap worker script (blob URL) with the shim URL
    // inlined. `include_str!` embeds the JS into the wasm binary — no
    // snippet file is emitted, so no rspack snippet-walking needed for it.
    let script = include_str!("js/worker_bootstrap.js").replace("WASM_BINDGEN_SHIM_URL", &shim_url);
    let arr = js_sys::Array::new();
    arr.set(0, JsValue::from_str(&script));
    let blob = web_sys::Blob::new_with_str_sequence(&arr)
        .expect("tur worker: failed to create bootstrap blob");
    // Module workers REJECT scripts served without a JS MIME type — a blob
    // built via `new Blob([str])` defaults to type "". Re-slice with
    // `text/javascript` so the module worker accepts it (classic workers
    // are lenient; module workers are not).
    let blob = blob
        .slice_with_f64_and_f64_and_content_type(0.0, blob.size(), "text/javascript")
        .expect("tur worker: failed to re-slice blob as text/javascript");
    // (Blob URL is intentionally not revoked — the worker fetches it
    // asynchronously; revoking risks a race. One URL per app is a
    // negligible leak for a long-lived worker. Matches wasm_thread.)
    let blob_url = web_sys::Url::create_object_url_with_blob(&blob)
        .expect("tur worker: failed to create blob URL");

    // Create the worker as a module worker so it can `import` the shim.
    // (Module workers are supported in all browsers that also support
    // `SharedArrayBuffer` + `crossOriginIsolated`, which this engine
    // already requires — so no polyfill is needed.)
    let options = web_sys::WorkerOptions::new();
    options.set_type(web_sys::WorkerType::Module);
    let worker = web_sys::Worker::new_with_options(&blob_url, &options)
        .expect("tur worker: failed to spawn worker");

    // Surface worker-side load/eval errors on the main console. Logged raw
    // (some failure modes — e.g. import resolution — produce events whose
    // typed fields aren't reliably string-convertible, so avoid
    // `ev.message()`).
    let onerr_closure =
        Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(|ev: web_sys::ErrorEvent| {
            let ev_val: JsValue = ev.into();
            let raw = js_sys::JSON::stringify(&ev_val)
                .ok()
                .and_then(|s| s.as_string())
                .unwrap_or_else(|| "<non-stringifiable ErrorEvent>".into());
            tracing::error!("[tur-main] worker error event: {raw}");
        });
    worker.set_onerror(Some(onerr_closure.as_ref().unchecked_ref()));
    onerr_closure.forget();

    // Box the factory + post `[module, memory, ptr]`. The pointer is valid
    // across threads (shared linear memory); the worker reconstitutes it.
    let entry = Box::new(WorkerEntry(factory));
    let ptr = Box::into_raw(entry) as u32;
    let init = js_sys::Array::new();
    init.push(&wasm_bindgen::module());
    init.push(&wasm_bindgen::memory());
    init.push(&JsValue::from_f64(ptr as f64));
    worker
        .post_message(&init)
        .expect("tur worker: failed to post init payload");

    worker
}

// ---------------------------------------------------------------------------
// Worker entry point (exported — called by the bootstrap script)
// ---------------------------------------------------------------------------

/// Worker entry point. Called by the bootstrap script after `init(module,
/// memory)` resolves. Reconstitutes the factory, builds the worker
/// scheduler view (on this thread), runs the factory → `loop_fut`, and
/// hands it to the cooperative mini-executor. Returns immediately after
/// setup — the worker stays alive via the `onmessage` wake handler + the
/// `setTimeout` repoll chain.
#[wasm_bindgen]
pub fn tur_worker_main(ptr: f64) {
    // SAFETY: `ptr` was produced by `Box::into_raw` on the main thread;
    // the wasm linear memory is shared, so the pointer is valid here.
    let entry = unsafe { Box::from_raw(ptr as u32 as *mut WorkerEntry) };
    let factory = entry.0;
    let worker_sched: Rc<dyn WorkerScheduler> = Rc::new(WasmWorkerScheduler);
    let loop_fut = factory(worker_sched);
    run_loop(loop_fut);
}

// ---------------------------------------------------------------------------
// Cooperative mini-executor
// ---------------------------------------------------------------------------

/// A waker that re-arms the `setTimeout(0)` repoll **when fired on the
/// worker thread**, and is a no-op otherwise. Cross-thread fires (mpsc send
/// from main) are handled by `postMessage` instead, so they must not touch
/// the worker's `thread_local` state (which would be unsound — the
/// `!Send` `loop_fut` lives there).
///
/// `Send + Sync` by construction: the only field is `ThreadId` (`Copy` +
/// `Send + Sync`). The `thread_local` is accessed exclusively on the
/// worker thread (guarded by the `ThreadId` check).
struct NoopWaker {
    worker_tid: ThreadId,
}

impl Wake for NoopWaker {
    fn wake(self: Arc<Self>) {
        Self::wake_by_ref(&self);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if std::thread::current().id() == self.worker_tid {
            schedule_repoll();
        }
        // Cross-thread: rely on main's `worker.postMessage(0)` instead.
    }
}

/// Install `loop_fut` + the wake handlers, then kick off the first poll.
pub(super) fn run_loop(loop_fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
    let state = Rc::new(ExecutorState {
        task: RefCell::new(Some(loop_fut)),
        scheduled: Cell::new(false),
        poll_fn: RefCell::new(None),
        done: Cell::new(false),
    });
    EXEC.with(|e| *e.borrow_mut() = Some(state.clone()));
    WORKER_TID.with(|t| t.set(Some(std::thread::current().id())));

    // The reused `setTimeout(0)` poll callback. Leaked (one per worker).
    let poll_closure = Closure::<dyn FnMut()>::new(poll_once);
    let poll_fn = poll_closure
        .as_ref()
        .unchecked_ref::<js_sys::Function>()
        .clone();
    poll_closure.forget();
    *state.poll_fn.borrow_mut() = Some(poll_fn);

    // Install the cross-thread wake handler on the worker global scope.
    // Main's `worker.postMessage(0)` fires this → `schedule_repoll`.
    let onmsg_closure = Closure::<dyn FnMut(MessageEvent)>::new(|_ev| schedule_repoll());
    let scope_ok = js_sys::global()
        .dyn_ref::<web_sys::DedicatedWorkerGlobalScope>()
        .map(|scope| {
            scope.set_onmessage(Some(onmsg_closure.as_ref().unchecked_ref()));
        })
        .is_some();
    if !scope_ok {
        tracing::error!(
            "tur_worker_main: not running on a DedicatedWorkerGlobalScope; \
             cross-thread wake will not fire"
        );
    }
    onmsg_closure.forget();

    // Kick off the first poll.
    schedule_repoll();
}

/// Arm a `setTimeout(0)` repoll (idempotent — no-op if one is already
/// armed or the executor is done). Called by the wake handler, the waker,
/// and the initial kick.
fn schedule_repoll() {
    EXEC.with(|e| {
        let guard = e.borrow();
        let Some(state) = guard.as_ref() else {
            return;
        };
        if state.done.get() {
            return;
        }
        if state.scheduled.replace(true) {
            return; // already armed
        }
        let poll_fn = match state.poll_fn.borrow().as_ref() {
            Some(f) => f.clone(),
            None => return, // not installed yet (only during run_loop setup)
        };
        drop(guard);
        schedule_set_timeout(&poll_fn, 0);
    });
}

/// Poll `loop_fut` once with a `NoopWaker`. The `setTimeout` callback body.
fn poll_once() {
    let state = match EXEC.with(|e| e.borrow().as_ref().cloned()) {
        Some(s) => s,
        None => return,
    };
    state.scheduled.set(false);
    if state.done.get() {
        return;
    }
    let worker_tid = WORKER_TID
        .with(|t| t.get())
        .expect("worker tid set during run_loop");
    let waker = Waker::from(Arc::new(NoopWaker { worker_tid }));
    let mut cx = Context::from_waker(&waker);
    let mut task_guard = state.task.borrow_mut();
    if let Some(fut) = task_guard.as_mut()
        && fut.as_mut().poll(&mut cx).is_ready()
    {
        state.done.set(true);
        *task_guard = None; // drop the completed future
    }
}

/// `setTimeout(cb, ms)` on the current global (`Window` or
/// `WorkerGlobalScope` — both expose `setTimeout`).
fn schedule_set_timeout(cb: &js_sys::Function, ms: i32) {
    let global = js_sys::global();
    let Ok(set_timeout) = Reflect::get(&global, &JsValue::from("setTimeout")) else {
        tracing::error!("schedule_set_timeout: global has no `setTimeout`");
        return;
    };
    let Some(set_timeout) = set_timeout.dyn_ref::<js_sys::Function>() else {
        tracing::error!("schedule_set_timeout: `setTimeout` is not a function");
        return;
    };
    if let Err(e) = set_timeout.call2(&global, cb, &JsValue::from(ms)) {
        tracing::error!("schedule_set_timeout failed: {e:?}");
    }
}
