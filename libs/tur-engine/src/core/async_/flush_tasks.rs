//! Flush-driven task queue for engine-internal async (`sleep`).
//!
//! `spawn_local`'d futures normally run on the platform executor
//! ([`WorkerContext::spawn_local`](crate::core::scheduler::WorkerContext) →
//! tokio `LocalSet` on native / `wasm_bindgen_futures::spawn_local` on wasm),
//! which polls them *between* frames — never inside `flush()`. For pure
//! engine-internal async (the `sleep(ms)` timer driver) that polling lag
//! breaks single-frame semantics: a `sleep(1000)` whose deadline is reached
//! by an `advance(1000)` only resolves *after* the flush triggered by that
//! advance has already returned, so the `.then` continuation doesn't run
//! until a later frame. The integration-test countdown cases (one pump per
//! second) never observe the tick.
//!
//! This queue closes that gap. `sleep` pushes its driver future here
//! instead of `worker_ctx.spawn_local`, and `flush()` polls every queued
//! task once per fixed-point iteration — so a sleep that becomes due
//! resolves *inside* the same flush, pushes its completion, which the same
//! flush drains (settling the promise and firing the `.then` reactions) —
//! all within one frame.
//!
//! Real platform async (HTTP, clipboard, file-picker) still uses
//! `worker_ctx.spawn_local`: those futures need the platform's I/O driver
//! (reqwest polling, web-sys promises) to make progress, and their
//! completions are already drained by `flush()` (the `Wake` they self-send
//! on completion re-arms the worker).
//!
//! ## Wakers
//!
//! Tasks are polled with a waker that sends `WorkerMsg::Wake` via the
//! shared `wake_worker` callback. The waker must be `Send + Sync` because
//! sleep futures register it with the test `VirtualClock`, which fires it
//! from the main thread. `flush` *also* re-polls every task unconditionally
//! each iteration, so a lost wake is harmless (the task is re-polled next
//! flush regardless) — the waker just minimises latency by ensuring a
//! flush runs when an external condition (timer, promise settlement)
//! becomes ready.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use crate::core::scheduler::{TaskHandle, track_spawn};

type BoxFuture = Pin<Box<dyn Future<Output = ()> + 'static>>;

/// A waker that fires the shared `wake_worker` callback (sends
/// `WorkerMsg::Wake`). `Send + Sync` so it can be stored in cross-thread
/// registries (e.g. the test `VirtualClock`'s timer map).
struct WakeWorker(Arc<dyn Fn() + Send + Sync>);

impl Wake for WakeWorker {
    fn wake(self: Arc<Self>) {
        (self.0)();
    }
}

fn make_waker(wake: Arc<dyn Fn() + Send + Sync>) -> Waker {
    Waker::from(Arc::new(WakeWorker(wake)))
}

/// Engine-internal task polled by `flush()`. Holds a `!Send` future (it
/// may capture `Rc` / boa objects) — polled only on the worker thread.
struct FlushTask {
    fut: RefCell<BoxFuture>,
}

/// Queue of engine-internal tasks driven by `flush()`. Held as
/// `Rc<FlushTaskQueue>` on `TurAppInternal`; bridges push via the
/// [`FlushTaskHandle`] (cheap `Rc` clone).
pub struct FlushTaskQueue {
    tasks: Rc<RefCell<Vec<Rc<FlushTask>>>>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl FlushTaskQueue {
    /// Construct with the shared `wake_worker` callback (sends
    /// `WorkerMsg::Wake`). Used both as the task waker (so an external
    /// ready-signal triggers a flush) and shared with [`CompletionQueue`].
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            tasks: Rc::new(RefCell::new(Vec::new())),
            wake,
        }
    }

    /// Cheap clone for bridges. Shares the task list.
    pub fn handle(&self) -> FlushTaskHandle {
        FlushTaskHandle {
            tasks: self.tasks.clone(),
        }
    }

    /// Poll every queued task once. Completed tasks are removed; tasks
    /// spawned mid-poll (e.g. a sleep pushed while another drains) are
    /// picked up next iteration / next flush. Returns the
    /// number that completed this pass — fed into `flush()`'s quiescence
    /// check so a completed task (which likely pushed a completion) keeps
    /// the fixed-point loop alive to drain it.
    pub fn poll_all(&self) -> usize {
        // Drain into a local vec so a task that spawns a sibling mid-poll
        // can push to the RefCell without a double-`borrow_mut` panic.
        let mut tasks = self.tasks.borrow_mut().drain(..).collect::<Vec<_>>();
        if tasks.is_empty() {
            return 0;
        }
        let waker = make_waker(self.wake.clone());
        let mut cx = Context::from_waker(&waker);
        let mut completed = 0;
        let mut i = 0;
        while i < tasks.len() {
            let task = tasks[i].clone();
            let done = {
                let mut fut = task.fut.borrow_mut();
                matches!(fut.as_mut().poll(&mut cx), Poll::Ready(()))
            };
            if done {
                tasks.swap_remove(i);
                completed += 1;
            } else {
                i += 1;
            }
        }
        // Re-insert surviving tasks ahead of any spawned during the poll,
        // so older tasks keep their polling order (FIFO-ish) and aren't
        // starved by freshly-spawned ones.
        let mut guard = self.tasks.borrow_mut();
        let spawned = guard.drain(..).collect::<Vec<_>>();
        tasks.extend(spawned);
        *guard = tasks;
        completed
    }

    /// True if no tasks are queued. (Currently unused externally but kept
    /// for diagnostics / future quiescence tuning.)
    pub fn is_empty(&self) -> bool {
        self.tasks.borrow().is_empty()
    }
}

/// Cheap handle held by bridges (`tur_sleep`). Shares the
/// task list with the parent [`FlushTaskQueue`].
pub struct FlushTaskHandle {
    tasks: Rc<RefCell<Vec<Rc<FlushTask>>>>,
}

impl FlushTaskHandle {
    /// Push a driver future to be polled by `flush()`. Returns a
    /// [`TaskHandle`] whose `abort()` cancels the task (its future is
    /// dropped at its next poll point) — the real timer abort behind
    /// `Task.cancel()` on a `sleep`. Dropping the handle detaches (the
    /// future keeps running to completion). The future runs across flush
    /// iterations (polled every iteration until it returns `Ready`).
    pub fn spawn(&self, fut: BoxFuture) -> TaskHandle {
        let tasks = self.tasks.clone();
        // `track_spawn` wraps `fut` in an `Abortable` + oneshot (so
        // `TaskHandle::abort`/`join` work identically to
        // `worker_ctx.spawn_local`) and hands the wrapped future to our
        // closure, which pushes it onto the flush-driven queue.
        track_spawn(fut, move |tracked| {
            tasks.borrow_mut().push(Rc::new(FlushTask {
                fut: RefCell::new(tracked),
            }));
        })
    }
}

impl Clone for FlushTaskHandle {
    fn clone(&self) -> Self {
        Self {
            tasks: self.tasks.clone(),
        }
    }
}
