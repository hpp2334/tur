//! Single-threaded async executor for tur.
//!
//! Isolates the `unsafe` `RawWaker` machinery behind a focused, dependency-
//! free API. The executor is strictly main-thread: all state is
//! `Rc<RefCell<...>>` (never `Arc`/`Mutex`), no cross-thread API surface.
//!
//! The only `unsafe` here is on [`WakerPayload`] — unavoidable because
//! `std::task::Waker: Send + Sync`. Sound because the executor is
//! single-threaded by construction: wakers are only ever woken from the same
//! thread that owns the `Rc`s. On wasm, browser microtasks that resolve
//! futures run on the same thread. On tests, everything is single-threaded
//! by design.
//!
//! The engine crate (`tur-engine`) wraps [`Executor`] and adds a
//! JS-specific completion queue on top — see
//! `tur_engine::core::async_::AsyncExecutor`.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context as TaskContext, Poll, RawWaker, RawWakerVTable, Waker};

/// Portable reactor primitives the engine cannot implement itself. Backends
/// inject an impl at engine build time. Intentionally tiny — anything with a
/// portable implementation belongs on [`Executor`] directly. Future additions:
/// `spawn_blocking`, `timer(Duration)`, etc.
pub trait AsyncRuntime: 'static {
    /// Wall-clock now. Distinct from boa's `FixedClock` (which is advanced
    /// deterministically by the embedder); used for wall-clock timestamps in
    /// async work.
    fn now(&self) -> std::time::Instant;
}

type BoxFuture = Pin<Box<dyn Future<Output = ()>>>;
type TaskId = u64;

/// Payload held inside a [`Waker`]. Carries a clone of the ready queue and
/// the task id, so when `wake()` fires from anywhere (microtask, inline
/// poll, etc.) the task gets re-enqueued for the next [`Executor::tick`].
struct WakerPayload {
    ready: Rc<RefCell<VecDeque<TaskId>>>,
    task_id: TaskId,
}

// SAFETY: `WakerPayload` holds `Rc<...>` which is `!Send + !Sync`. We assert
// `Send + Sync` because `std::task::Waker` requires it. Sound because the
// executor is strictly single-threaded: wakers are only ever woken from the
// main thread (the same thread that owns the `Rc`s). On wasm, browser
// microtasks that resolve futures run on the same thread. On tests,
// everything is single-threaded by design.
unsafe impl Send for WakerPayload {}
unsafe impl Sync for WakerPayload {}

const WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    waker_clone,
    waker_wake,
    waker_wake_by_ref,
    waker_drop,
);

fn make_waker(payload: WakerPayload) -> Waker {
    let ptr = Box::into_raw(Box::new(payload));
    // SAFETY: the vtable functions are sound for the boxed `WakerPayload`
    // pointer; see their individual safety comments.
    unsafe { Waker::from_raw(RawWaker::new(ptr as *const (), &WAKER_VTABLE)) }
}

unsafe fn waker_clone(ptr: *const ()) -> RawWaker {
    // SAFETY: caller upholds that `ptr` is a valid boxed `WakerPayload`.
    let payload = unsafe { &*(ptr as *const WakerPayload) };
    let cloned = WakerPayload {
        ready: payload.ready.clone(),
        task_id: payload.task_id,
    };
    let boxed = Box::new(cloned);
    RawWaker::new(Box::into_raw(boxed) as *const (), &WAKER_VTABLE)
}

unsafe fn waker_wake(ptr: *const ()) {
    // SAFETY: caller upholds that `ptr` is a valid boxed `WakerPayload`.
    // `Box::from_raw` consumes the box (matching `wake`'s drop-ownership
    // contract) and re-enqueues the task id.
    let payload = unsafe { Box::from_raw(ptr as *mut WakerPayload) };
    payload.ready.borrow_mut().push_back(payload.task_id);
}

unsafe fn waker_wake_by_ref(ptr: *const ()) {
    // SAFETY: caller upholds that `ptr` is a valid boxed `WakerPayload`.
    // We do NOT consume the box; just read the fields and re-enqueue.
    let payload = unsafe { &*(ptr as *const WakerPayload) };
    payload.ready.borrow_mut().push_back(payload.task_id);
}

unsafe fn waker_drop(ptr: *const ()) {
    // SAFETY: caller upholds that `ptr` is a valid boxed `WakerPayload`
    // whose ref count has dropped to zero.
    unsafe { drop(Box::from_raw(ptr as *mut WakerPayload)) };
}

/// Engine-owned, engine-driven single-threaded executor with real wakers.
///
/// Held as `Rc<Executor>` and exposed to spawned futures (which can capture
/// the `Rc` to spawn nested tasks). The JS-binding wrapper in
/// `tur-engine::core::async_` adds a Completion queue on top for settling
/// JsPromises under `&mut boa_engine::Context`.
#[derive(Default)]
pub struct Executor {
    /// Live futures keyed by id. Removed when a future returns `Ready`.
    /// Held as `Rc<RefCell<...>>` so wakers can outlive the borrowed
    /// `&Executor` used during `tick`.
    tasks: Rc<RefCell<HashMap<TaskId, BoxFuture>>>,
    /// Task ids ready to be polled. Populated by `spawn` (initial enqueue)
    /// and by wakers (re-enqueue on wake). Drained by `tick`.
    ready: Rc<RefCell<VecDeque<TaskId>>>,
    /// Monotonic task id source.
    next_id: Rc<AtomicU64>,
}

impl Executor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn a `!Send` future. The future lives until it returns `Ready` or
    /// the executor is dropped — there is no explicit cancellation API
    /// (matches the fire-and-forget pattern used by clipboard/http bridge fns).
    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.tasks.borrow_mut().insert(id, Box::pin(fut));
        self.ready.borrow_mut().push_back(id);
    }

    /// Alias for [`Self::spawn`]; spelling preserved for callers that
    /// originally came from an executor API where spawn returned a Task.
    pub fn spawn_detached<F>(&self, fut: F)
    where
        F: Future<Output = ()> + 'static,
    {
        self.spawn(fut);
    }

    /// Drive all ready tasks one poll step. Returns `true` if any task was
    /// polled.
    ///
    /// A task that returns `Pending` parks; its waker (produced by
    /// [`make_waker`]) re-enqueues the task id on wake. A task that returns
    /// `Ready` is dropped and removed from the registry.
    pub fn tick(&self) -> bool {
        let ids: Vec<TaskId> = self.ready.borrow_mut().drain(..).collect();
        if ids.is_empty() {
            return false;
        }
        for id in ids {
            // Take the future out of the registry before polling, so the
            // registry can be safely re-borrowed by wakers / nested spawns
            // inside the poll.
            let mut fut = match self.tasks.borrow_mut().remove(&id) {
                Some(f) => f,
                None => continue, // task was already removed (cancel/drop)
            };
            let waker = make_waker(WakerPayload {
                ready: self.ready.clone(),
                task_id: id,
            });
            let mut cx = TaskContext::from_waker(&waker);
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(()) => {
                    // Task complete — do not reinsert.
                }
                Poll::Pending => {
                    self.tasks.borrow_mut().insert(id, fut);
                }
            }
        }
        true
    }

    /// True if there is pending work (ready tasks or live tasks).
    pub fn has_pending(&self) -> bool {
        !self.ready.borrow().is_empty() || !self.tasks.borrow().is_empty()
    }
}
