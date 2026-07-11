//! Engine-side wrapper around [`tur_async::Executor`] that adds a
//! JS-specific completion queue.
//!
//! The unsafe `RawWaker` machinery lives in `tur-async` (focused, deps-free).
//! This module adds the bridge to boa's `Context`: spawned futures produce
//! [`Completion`] closures (which settle `JsPromise`s under `&mut Context`)
//! and push them into [`AsyncExecutor`]'s queue. `flush` drains the queue
//! between `tick` and boa's microtask drain — completions enqueue
//! PromiseJobs that the boa drain then runs.
//!
//! ## Flush loop
//!
//! ```text
//! loop {
//!     async_progress = async_executor.tick();              // poll Rust futures
//!     async_executor.drain_completions(boa);               // settle promises
//!     jobs_run = executor.drain(boa);                      // run PromiseJobs
//!     // …events, reactive, layout, mutations…
//!     if all quiet && !async_pending { break; }
//! }
//! ```
//!
//! See [`crate::core::app::TurAppInternal::flush`] for the full sequence.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::rc::Rc;

use boa_engine::Context;
use boa_engine::JsResult;

pub use tur_async::AsyncRuntime;

/// A closure that runs under `&mut Context` to settle a JsPromise (or any
/// other synchronous side-effect that needs Context access). Produced by a
/// spawned future's completion and drained by [`AsyncExecutor::drain_completions`]
/// inside `flush`.
pub type Completion = Box<dyn FnOnce(&mut Context) -> JsResult<()>>;

/// Engine-side async executor: a [`tur_async::Executor`] (which handles
/// futures, wakers, ready queue) plus a [`Completion`] queue (which bridges
/// to boa's `Context`).
///
/// Held as `Rc<AsyncExecutor>` on
/// [`crate::core::app::TurAppInternal`] and exposed to plugins via
/// [`crate::core::plugin::PluginContext`]. Spawned futures can capture
/// `Rc<AsyncExecutor>` to call [`Self::complete`] or [`Self::spawn`].
#[derive(Default)]
pub struct AsyncExecutor {
    inner: tur_async::Executor,
    completions: Rc<RefCell<VecDeque<Completion>>>,
}

impl AsyncExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn a `!Send` future. The future lives until it returns `Ready` or
    /// the engine is dropped.
    ///
    /// The future may capture `Rc<AsyncExecutor>` (cheap clone) to call
    /// [`Self::complete`] or [`Self::spawn`] from inside its body.
    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + 'static,
    {
        self.inner.spawn(fut);
    }

    /// Alias for [`Self::spawn`]; spelling preserved for callers that
    /// originally came from an executor API where spawn returned a Task.
    pub fn spawn_detached<F>(&self, fut: F)
    where
        F: Future<Output = ()> + 'static,
    {
        self.spawn(fut);
    }

    /// Push a completion closure. Called from inside a spawned future when it
    /// has produced a Rust result and needs to settle a JsPromise (or similar)
    /// under `&mut Context`. The closure runs on the next
    /// [`Self::drain_completions`] pass inside `flush`.
    pub fn complete(&self, f: Completion) {
        self.completions.borrow_mut().push_back(f);
    }

    /// Drive all ready tasks one poll step. Returns `true` if any task was
    /// polled. Delegates to [`tur_async::Executor::tick`].
    pub fn tick(&self) -> bool {
        self.inner.tick()
    }

    /// Drain pending completions under `&mut Context`. Called inside `flush`
    /// after `tick` and before boa's `executor.drain` — completions settle
    /// JsPromises, which enqueues PromiseJobs that the boa drain then runs.
    pub fn drain_completions(&self, ctx: &mut Context) {
        let cs: Vec<Completion> = self.completions.borrow_mut().drain(..).collect();
        for f in cs {
            if let Err(e) = f(ctx) {
                tracing::error!("async completion error: {e}");
            }
        }
    }

    /// True if there is pending work (ready tasks, live tasks, or pending
    /// completions). Used by `flush`'s termination condition — see
    /// [`crate::core::app::TurAppInternal::flush`].
    pub fn has_pending(&self) -> bool {
        self.inner.has_pending() || !self.completions.borrow().is_empty()
    }
}
