//! [`AsyncWorkerContext`] — the async handle handed to worker-side spawned
//! tasks.
//!
//! The worker does not expose its raw [`WorkerScheduler`](crate::core::scheduler::WorkerScheduler)
//! to spawn sites. Instead [`TurInstanceContext::spawn_local`](crate::core::js_runtime::TurInstanceContext::spawn_local)
//! passes an `AsyncWorkerContext` into the task closure, providing:
//! - [`AsyncWorkerContext::sleep`] — platform timer,
//! - [`AsyncWorkerContext::spawn_local`] — nested spawn,
//! - [`AsyncWorkerContext::request_paint`] — the worker's self-waking paint
//!   signal (sets `need_paint` and, if the worker is idle, emits a coalesced
//!   `WorkerMsg::Wake` so the worker's own loop pumps a flush).
//!
//! This is the canonical home for deferred, timer-driven paints (e.g. the
//! caret-blink loop): the task sleeps, then calls `request_paint`, and the
//! worker re-arms itself — no main involvement, no raw scheduler access.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::core::js_runtime::TurInstanceContext;
use crate::core::scheduler::{Sleep, TaskHandle};

type BoxFuture = Pin<Box<dyn Future<Output = ()> + 'static>>;

/// Worker-side async context handed to a spawned task closure. Cheap to
/// clone (Rc-backed); moves into the task's future so the task can drive
/// timers, nest spawns, and signal paints without touching the raw
/// scheduler or the `need_paint` flag directly.
#[derive(Clone)]
pub struct AsyncWorkerContext {
    pub(crate) js_ctx: TurInstanceContext,
}

impl AsyncWorkerContext {
    pub(crate) fn new(js_ctx: TurInstanceContext) -> Self {
        Self { js_ctx }
    }

    /// Sleep for `d` (engine time). Backed by the worker scheduler's
    /// platform-specific timer (`setTimeout` on wasm, `tokio::time::sleep`
    /// on native, virtual clock in tests).
    pub fn sleep(&self, d: Duration) -> Sleep {
        self.js_ctx.worker_sched().sleep(d)
    }

    /// Spawn a nested worker-side task on the platform executor. Returns a
    /// [`TaskHandle`] whose `abort()` cancels it (drops the future at its
    /// next `.await`).
    pub fn spawn_local(&self, fut: BoxFuture) -> TaskHandle {
        self.js_ctx.worker_sched().spawn_local(fut)
    }

    /// Mark this frame paint-worthy and, if the worker is idle, emit a
    /// coalesced self-wake so the worker's own loop pumps a flush. The
    /// canonical signal for a deferred paint (e.g. a caret-blink tick).
    pub fn request_paint(&self) {
        self.js_ctx.request_paint();
    }
}
