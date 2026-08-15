//! Wasm-backed scheduler driver.
//!
//! Implements [`MainSchedulerDriver`] (main thread) and
//! [`WorkerSchedulerDriver`] (worker thread, via [`WasmWorkerScheduler`])
//! for the wasm32 target. The runtime builder wraps the main driver in a
//! `MainScheduler` view via `.scheduler(driver)`; the per-worker driver is
//! a separate stateless object constructed inside each worker.
//!
//! ## Worker pools (multi-tenant workers)
//!
//! `spawn_worker_in` hosts apps on per-pool Web Workers: a pool grows up
//! to its `max_threads` workers (first apps each get a fresh worker),
//! then additional apps are delivered into the least-loaded existing
//! worker of that pool as a **factory message** — the worker hosts
//! multiple app `worker_loop` futures cooperatively on its JS event loop
//! (see [`crate::worker_spawn`]). Apps in different pools never share a
//! worker; a cap ≥ the app count degenerates to one-worker-per-app.
//!
//! Message disambiguation on the worker's `onmessage`:
//! - `[module, memory, ptr]` (Array) — the FIRST factory, consumed by the
//!   bootstrap script before Rust installs its handler.
//! - `{ t: "tur-factory", ptr }` (Object) — an additional factory for an
//!   already-running worker (delivered by this driver).
//! - `0` (number) — a cross-thread wake kick.
//!
//! ## Vsync events
//!
//! `request_vsync` arms a `requestAnimationFrame` callback (idempotent —
//! no-op if a rAF is already pending). The rAF closure is driver-owned
//! (constructed once in [`WasmSchedulerDriver::new`]); on fire it pushes
//! an event into the subscribed `vsync_tx` channel. The engine subscribes
//! once at `run_loop` startup via `vsync_events()`.
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
//! never parks in a sync `Atomics.wait`.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use js_sys::{Object, Reflect};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use tur_engine::core::scheduler::{
    MainSchedulerDriver, Sleep, TaskHandle, VsyncEvents, WorkerFactory, WorkerHandle,
    WorkerPoolHandle, WorkerSchedulerDriver, track_spawn,
};

use crate::worker_spawn;

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
    /// Worker-pool registry: pool → hosted workers (+ live-app counts).
    /// Main-thread only (the driver is `!Send`). Workers are reaped
    /// (terminated + removed) lazily when a later spawn finds them at
    /// zero live apps.
    pools: RefCell<Vec<PoolEntry>>,
}

/// One pool's registry entry.
struct PoolEntry {
    handle: WorkerPoolHandle,
    workers: Vec<PoolWorker>,
}

/// One hosted Web Worker + its live-app count. `live` is `Rc<Cell<usize>>`
/// so the `WorkerHandle`'s join closure can decrement it.
struct PoolWorker {
    worker: Rc<web_sys::Worker>,
    live: Rc<Cell<usize>>,
}

impl WasmSchedulerDriver {
    /// Construct a new wasm scheduler driver. Allocates a long-lived
    /// rAF closure that fires vsync events into the subscribed channel.
    pub fn new() -> Rc<Self> {
        let inner = Rc::new(WasmInner {
            vsync_txs: RefCell::new(Vec::new()),
            raf_id: RefCell::new(None),
            raf_closure: RefCell::new(None),
            pools: RefCell::new(Vec::new()),
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

impl MainSchedulerDriver for WasmSchedulerDriver {
    fn spawn_worker_in(&self, pool: &WorkerPoolHandle, factory: WorkerFactory) -> WorkerHandle {
        // Reap workers whose last app finished (terminate + remove) —
        // a lazy exit, mirroring the native registry's lane reaping.
        let mut pools = self.inner.pools.borrow_mut();
        let entry = match pools.iter_mut().find(|e| e.handle.ptr_eq(pool)) {
            Some(entry) => entry,
            None => {
                // First spawn into this pool — host it on demand (pool
                // registration/identity was validated by the engine).
                pools.push(PoolEntry {
                    handle: pool.clone(),
                    workers: Vec::new(),
                });
                pools.last_mut().expect("just pushed")
            }
        };
        let mut i = 0;
        while i < entry.workers.len() {
            if entry.workers[i].live.get() == 0 {
                let dead = entry.workers.remove(i);
                dead.worker.terminate();
            } else {
                i += 1;
            }
        }

        let pool_worker = if entry.workers.len() < pool.max_threads() {
            // Grow: this app gets a fresh worker (the factory travels in
            // the worker's init payload — the existing spawn path).
            let worker = Rc::new(worker_spawn::spawn(factory));
            PoolWorker {
                worker,
                live: Rc::new(Cell::new(1)),
            }
        } else {
            // Cap reached: deliver the factory into the least-loaded
            // existing worker as a tagged message; its onmessage handler
            // builds the app loop and hosts it cooperatively.
            let host = entry
                .workers
                .iter()
                .min_by_key(|w| w.live.get())
                .expect("cap >= 1 guarantees a live worker");
            let ptr = Box::into_raw(Box::new(worker_spawn::WorkerEntry(factory))) as u32;
            let msg = Object::new();
            Reflect::set(
                &msg,
                &JsValue::from_str("t"),
                &JsValue::from_str("tur-factory"),
            )
            .expect("set factory tag");
            Reflect::set(
                &msg,
                &JsValue::from_str("ptr"),
                &JsValue::from_f64(ptr as f64),
            )
            .expect("set factory ptr");
            host.worker
                .post_message(&msg)
                .expect("tur pool: failed to post factory payload");
            host.live.set(host.live.get() + 1);
            PoolWorker {
                worker: host.worker.clone(),
                live: host.live.clone(),
            }
        };
        let live = pool_worker.live.clone();
        // The worker Rc is captured by BOTH closures (notify + join) as
        // well as the registry — three references, so the Worker is never
        // GC'd while the backend or the registry might still wake it.
        let worker_rc = pool_worker.worker.clone();
        entry.workers.push(pool_worker);
        drop(pools);

        // Cross-thread wake: `worker.postMessage(0)`. Wakes ALL loops on
        // that worker; each drains its own mpsc and no-ops when empty.
        let worker_for_notify = worker_rc.clone();
        let notify: Rc<dyn Fn()> = Rc::new(move || {
            let _ = worker_for_notify.post_message(&JsValue::from(0i32));
        });
        let worker_for_join = worker_rc;
        let join: Box<dyn FnOnce()> = Box::new(move || {
            // Join = this app's slot released; terminate the worker when
            // the last app is gone (in practice join is never called by
            // the engine — MainBackend holds the handle for its lifetime —
            // so reaping normally happens lazily at the next spawn).
            if live.get() > 0 {
                live.set(live.get() - 1);
            }
            if live.get() == 0 {
                worker_for_join.terminate();
            }
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

/// Worker-side scheduler driver — zero state, constructed inside
/// [`crate::worker_spawn::tur_worker_main`] on the worker thread (wrapped in
/// a [`WorkerScheduler`] view). Methods delegate to global wasm primitives
/// (`spawn_local`, `setTimeout`-backed `sleep`) that work on any JS thread.
pub(crate) struct WasmWorkerScheduler;

impl WorkerSchedulerDriver for WasmWorkerScheduler {
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
