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
//! oneshot channel.
//!
//! ## Spawn primitives
//!
//! `spawn_local` delegates to `wasm_bindgen_futures::spawn_local` (works
//! on both the main thread and Web Workers — both drive the JS event
//! loop). `block_on` delegates to `futures::executor::block_on` (workers
//! can block; the main thread cannot — `block_on` is worker-only by
//! convention).
//!
//! ## Worker spawn
//!
//! `spawn_worker` uses `wasm_thread::spawn` to create a Web Worker. The
//! worker side has no shared state (all primitives are global wasm APIs),
//! so the worker view is a zero-state [`WasmWorkerScheduler`].

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use tur_engine::core::scheduler::{
    MainScheduler, Sleep, VsyncEvents, WorkerHandle, WorkerScheduler,
};

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
    fn spawn_worker(
        &self,
        factory: Box<dyn FnOnce(Rc<dyn WorkerScheduler>) + Send + 'static>,
    ) -> WorkerHandle {
        // `wasm_thread::spawn` requires the closure to be `Send + 'static`.
        // We can't pass an `Rc<dyn WorkerScheduler>` across — but the wasm
        // worker side has no shared state anyway: all primitives
        // (`spawn_local`, `block_on`, `sleep`) delegate to global wasm
        // primitives that work on any thread. So the worker view is a
        // zero-state `WasmWorkerScheduler`.
        let _join_handle = wasm_thread::spawn(move || {
            let worker_view: Rc<dyn WorkerScheduler> = Rc::new(WasmWorkerScheduler);
            factory(worker_view);
        });
        WorkerHandle::new(Box::new(|| {
            // wasm_thread doesn't expose a join in the std::thread sense —
            // the Web Worker terminates when its closure returns.
        }))
    }

    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
        wasm_bindgen_futures::spawn_local(fut);
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
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
        wasm_bindgen_futures::spawn_local(fut);
    }

    fn block_on(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
        futures::executor::block_on(fut);
    }

    fn sleep(&self, d: Duration) -> Sleep {
        wasm_sleep(d)
    }
}

/// Internal worker-only scheduler — zero state, used by `spawn_worker`.
/// This is kept for potential future use but `WasmSchedulerDriver` itself
/// implements `WorkerScheduler` (above), so this struct is currently unused.
#[allow(dead_code)]
struct WasmWorkerScheduler;

impl WorkerScheduler for WasmWorkerScheduler {
    fn spawn_local(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
        wasm_bindgen_futures::spawn_local(fut);
    }

    fn block_on(&self, fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
        futures::executor::block_on(fut);
    }

    fn sleep(&self, d: Duration) -> Sleep {
        wasm_sleep(d)
    }
}

/// Shared `sleep` implementation used by both `WasmSchedulerDriver`
/// (main-side) and `WasmWorkerScheduler` (worker-side). Backed by
/// `setTimeout` + an oneshot channel.
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
    if let Some(window) = web_sys::window() {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            ms.max(1),
        );
    }
    // Leak the Closure so setTimeout can call it. The closure's captured
    // state (the oneshot sender) is consumed on fire, so the leaked memory
    // is one Closure per sleep. Acceptable for typical use.
    closure.forget();
    Sleep(Box::pin(async move {
        let _ = rx.await;
    }))
}
