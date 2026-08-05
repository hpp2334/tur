//! Wasm-backed scheduler driver.
//!
//! Implements both [`MainScheduler`] and [`WorkerScheduler`] for the wasm32
//! target. The same driver object backs both trait objects stored on
//! `TurRuntime` (set together via `TurRuntimeBuilder::scheduler(driver)`).
//!
//! ## Vsync events
//!
//! `request_vsync` arms a `requestAnimationFrame` callback (idempotent —
//! no-op if a rAF is already pending). The rAF closure is driver-owned
//! (constructed once in [`WasmSchedulerDriver::new`]); on fire it pushes
//! an event into the subscribed `vsync_tx` channel. The engine subscribes
//! once at `start_loop` startup via `vsync_events()`.
//!
//! ## Sleep
//!
//! `sleep(d)` returns a `Sleep(BoxFuture)` backed by `setTimeout` + an
//! oneshot channel. `setTimeout` is resolved off `js_sys::global()` so it
//! works on BOTH the main thread (`Window`) and the worker
//! (`DedicatedWorkerGlobalScope`) — `web_sys::window()` returns `None` on a
//! worker, which would silently drop the timer and never resolve `sleep`.
//!
//! ## Spawn primitives
//!
//! `spawn_local` delegates to `wasm_bindgen_futures::spawn_local` (works
//! on both the main thread and Web Workers — both drive a JS event loop).
//! Side-futures (HTTP/clipboard/file-picker) run freely because the worker
//! no longer parks in a sync `Atomics.wait` (see below).
//!
//! ## Worker spawn
//!
//! `spawn_worker` uses the in-tree spawner ([`crate::worker_spawn::spawn`])
//! — NOT `wasm_thread` — to create a shared-memory Web Worker whose
//! bootstrap script does NOT call `close()`, and to keep the `Worker`
//! handle so main can `worker.postMessage(0)` to wake an idle worker
//! cross-thread (the only way to kick a Web Worker's JS event loop without
//! a sync `Atomics.wait`, which would freeze the loop). The worker drives
//! its `loop_fut` cooperatively in [`crate::worker_spawn`] (mini-executor +
//! `NoopWaker`). See that module's docs for the full driving model.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use js_sys::Reflect;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use tur_engine::core::scheduler::{
    MainScheduler, Sleep, TaskHandle, VsyncEvents, WorkerHandle, WorkerScheduler, track_spawn,
};

use crate::worker_spawn::{self, LoopFactory};

/// Wasm-backed scheduler driver. Construct via [`WasmSchedulerDriver::new`].
pub struct WasmSchedulerDriver {
    inner: Rc<WasmInner>,
}

struct WasmInner {
    /// All vsync event subscribers. Each `vsync_events()` call pushes a
    /// new sender; `fire_vsync` pushes to ALL. Supports multi-instance
    /// (multiple TurApps from one runtime).
    vsync_txs: RefCell<Vec<futures::channel::mpsc::UnboundedSender<()>>>,
    /// Pending rAF handle, if any. `None` ⇒ no rAF outstanding. Cleared by
    /// the rAF closure when it fires.
    raf_id: RefCell<Option<i32>>,
    /// Driver-owned rAF closure. Constructed once in `new` (with a
    /// `Rc<WasmInner>` capture); the JS engine holds a reference for as
    /// long as a rAF is pending. Re-armed each `request_vsync` call.
    raf_closure: RefCell<Option<Closure<dyn Fn()>>>,
}

impl WasmSchedulerDriver {
    /// Construct a new wasm scheduler driver. Allocates a long-lived
    /// rAF closure that fires vsync events into the subscribed channel.
    pub fn new() -> Rc<Self> {
        let inner = Rc::new(WasmInner {
            vsync_txs: RefCell::new(Vec::new()),
            raf_id: RefCell::new(None),
            raf_closure: RefCell::new(None),
        });

        // Build the rAF closure that fires a vsync event. Captures the
        // inner Rc by clone. Held across rAF requests via RefCell.
        let inner_for_closure = inner.clone();
        let raf_closure = Closure::<dyn Fn()>::new(move || {
            inner_for_closure.fire_vsync();
        });
        *inner.raf_closure.borrow_mut() = Some(raf_closure);

        Rc::new(Self { inner })
    }
}

impl WasmInner {
    fn fire_vsync(&self) {
        // Clear the pending handle (rAF is one-shot).
        *self.raf_id.borrow_mut() = None;
        // Push an event to ALL subscribers.
        for tx in self.vsync_txs.borrow().iter() {
            let _ = tx.unbounded_send(());
        }
    }
}

impl MainScheduler for WasmSchedulerDriver {
    fn spawn_worker(&self, factory: LoopFactory) -> WorkerHandle {
        // Spawn a shared-memory Web Worker via the in-tree spawner. The
        // worker's bootstrap does NOT `close()` on entry-return, so it
        // stays alive while its JS event loop has pending tasks (the
        // `onmessage` wake handler + the `setTimeout` repoll chain). We
        // keep the `Worker` handle so main can `postMessage(0)` to wake
        // it cross-thread.
        let worker = worker_spawn::spawn(factory);

        // Cross-thread wake: `worker.postMessage(0)`. Held alive for the
        // app's lifetime by both the `notify` Rc (MainBackend) and the
        // `join` closure (the `_worker_handle` field) — two references so
        // the Worker isn't GC'd if either drops.
        let worker_for_notify = worker.clone();
        let notify: Rc<dyn Fn()> = Rc::new(move || {
            let _ = worker_for_notify.post_message(&JsValue::from(0i32));
        });
        let worker_for_join = worker;
        let join: Box<dyn FnOnce()> = Box::new(move || {
            // terminate the worker if MainBackend is ever dropped (it
            // never is in practice — TurApp lives for the page lifetime).
            worker_for_join.terminate();
        });
        WorkerHandle::with_notify(join, notify)
    }

    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        track_spawn(fut, wasm_bindgen_futures::spawn_local)
    }

    fn vsync_events(&self) -> VsyncEvents {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        self.inner.vsync_txs.borrow_mut().push(tx);
        VsyncEvents(rx)
    }

    fn request_vsync(&self) {
        // Idempotent: no-op if a rAF is already pending.
        if self.inner.raf_id.borrow().is_some() {
            return;
        }
        let Some(window) = web_sys::window() else {
            return;
        };
        let raf_closure_ref = self.inner.raf_closure.borrow();
        let Some(closure) = raf_closure_ref.as_ref() else {
            return;
        };
        let id = window
            .request_animation_frame(closure.as_ref().unchecked_ref())
            .unwrap_or(-1);
        if id >= 0 {
            *self.inner.raf_id.borrow_mut() = Some(id);
        }
    }

    fn sleep(&self, d: Duration) -> Sleep {
        wasm_sleep(d)
    }
}

/// `WasmSchedulerDriver` also implements `WorkerScheduler` so it can be
/// passed via `TurRuntimeBuilder::scheduler(driver)`. The worker methods
/// are identical to the main methods (wasm primitives work on any thread).
impl WorkerScheduler for WasmSchedulerDriver {
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        track_spawn(fut, wasm_bindgen_futures::spawn_local)
    }

    fn sleep(&self, d: Duration) -> Sleep {
        wasm_sleep(d)
    }
}

/// Worker-side scheduler view — zero state, constructed inside
/// [`crate::worker_spawn::tur_worker_main`] on the worker thread. Methods
/// delegate to global wasm primitives (`spawn_local`, `setTimeout`-backed
/// `sleep`) that work on any JS thread.
pub(crate) struct WasmWorkerScheduler;

impl WorkerScheduler for WasmWorkerScheduler {
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) -> TaskHandle {
        track_spawn(fut, wasm_bindgen_futures::spawn_local)
    }

    fn sleep(&self, d: Duration) -> Sleep {
        wasm_sleep(d)
    }
}

/// Shared `sleep` implementation used by both `WasmSchedulerDriver`
/// (main-side) and `WasmWorkerScheduler` (worker-side). Backed by
/// `setTimeout` + an oneshot channel.
///
/// Resolves `setTimeout` off `js_sys::global()` (not `web_sys::window()`)
/// so it works on a `DedicatedWorkerGlobalScope` — `window()` returns
/// `None` on a worker, which would silently drop the timer and the
/// `sleep()`/`launch()` it backs would never resolve.
fn wasm_sleep(d: Duration) -> Sleep {
    let (tx, rx) = futures::channel::oneshot::channel();
    let ms = d.as_millis().min(i32::MAX as u128) as i32;
    let tx = Rc::new(RefCell::new(Some(tx)));
    let tx_clone = tx.clone();
    let closure = Closure::<dyn FnMut()>::new(move || {
        if let Some(t) = tx_clone.borrow_mut().take() {
            let _ = t.send(());
        }
    });
    // `setTimeout` lives on both `Window` and `WorkerGlobalScope`; fetch it
    // off the current global so this works on either thread.
    let global = js_sys::global();
    if let Some(set_timeout) = Reflect::get(&global, &JsValue::from("setTimeout"))
        .ok()
        .and_then(|v| v.dyn_into::<js_sys::Function>().ok())
    {
        let _ = set_timeout.call2(
            &global,
            closure.as_ref().unchecked_ref(),
            &JsValue::from(ms.max(1)),
        );
    } else {
        tracing::error!("wasm_sleep: global has no `setTimeout`");
    }
    // Leak the Closure so setTimeout can call it. The closure's captured
    // state (the oneshot sender) is consumed on fire, so the leaked memory
    // is one Closure per sleep. Acceptable for typical use.
    closure.forget();
    Sleep(Box::pin(async move {
        let _ = rx.await;
    }))
}
