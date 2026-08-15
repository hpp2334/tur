//! Engine-side completion queue — the boa-bridge half of the old `AsyncExecutor`.
//!
//! Spawned futures (clipboard reads, http requests, file-picker dialogs,
//! caret blink tasks) need to settle `JsPromise`s under `&mut Context`.
//! The completion queue carries closures from the future's completion
//! (running on a `WorkerContext`'s executor) to the engine's `flush()`
//! loop, where they're drained under `&mut Context`.
//!
//! ## Wake-on-push
//!
//! When a future pushes a completion, the queue fires its `on_push`
//! callback. The engine wires this to send `WorkerMsg::Wake` to the worker
//! (a self-send via the worker's incoming channel clone), so the worker
//! flushes promptly to drain the completion. Without this, an idle worker
//! would never wake to drain a completion that arrives between frames.
//!
//! ## Vs. the old `AsyncExecutor`
//!
//! The old `AsyncExecutor` was both a future poller AND a completion queue.
//! With the new scheduler model, future polling moves to the driver
//! (`WorkerContext::spawn_local` → embedder's executor); only the
//! completion queue remains, narrow and focused.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use boa_engine::Context;

// `Completion` is the same closure type the existing `AsyncExecutor::complete`
// API uses — re-exported from `super` to keep a single canonical definition
// during the transition.
use super::Completion;

/// Engine-side completion queue. Held as `Rc<CompletionQueue>` on
/// [`crate::core::app::TurAppInternal`]; bridges receive a
/// [`CompletionHandle`] (cheap `Rc` clone) to push completions from inside
/// spawned futures.
pub struct CompletionQueue {
    pending: Rc<RefCell<VecDeque<Completion>>>,
    on_push: Arc<dyn Fn() + Send + Sync>,
}

impl std::fmt::Debug for CompletionQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletionQueue")
            .field("pending_count", &self.pending.borrow().len())
            .finish_non_exhaustive()
    }
}

impl CompletionQueue {
    /// Construct with an `on_push` callback. The engine wires this to
    /// send `WorkerMsg::Wake` to the worker, ensuring the worker flushes
    /// promptly whenever a future completes. Shared with the flush-driven
    /// task queue (same `Arc<dyn Fn() + Send + Sync>`).
    pub fn new(on_push: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            pending: Rc::new(RefCell::new(VecDeque::new())),
            on_push,
        }
    }

    /// Cheap clone for bridges to capture. The handle shares the underlying
    /// queue + `on_push` callback.
    pub fn handle(&self) -> CompletionHandle {
        CompletionHandle {
            pending: self.pending.clone(),
            on_push: self.on_push.clone(),
        }
    }

    /// Drain pending completions under `&mut Context`. Called inside
    /// `flush()` after polling the worker scheduler's executor.
    pub fn drain(&self, ctx: &mut Context) {
        let completions: Vec<Completion> = self.pending.borrow_mut().drain(..).collect();
        for completion in completions {
            if let Err(e) = completion(ctx) {
                tracing::error!("completion error: {e}");
            }
        }
    }

    /// True if there are pending completions. Used by `flush()`'s
    /// termination condition.
    pub fn has_pending(&self) -> bool {
        !self.pending.borrow().is_empty()
    }
}

/// Cheap handle held by spawned futures. Shares the underlying queue +
/// `on_push` callback with the parent [`CompletionQueue`].
pub struct CompletionHandle {
    pending: Rc<RefCell<VecDeque<Completion>>>,
    on_push: Arc<dyn Fn() + Send + Sync>,
}

impl CompletionHandle {
    /// Push a completion closure. Called from inside a spawned future when
    /// it has produced a Rust result and needs to settle a `JsPromise` (or
    /// similar) under `&mut Context`. Fires `on_push`, which sends
    /// `WorkerMsg::Wake` to the worker so the next `flush()` drains this.
    pub fn push(&self, completion: Completion) {
        self.pending.borrow_mut().push_back(completion);
        (self.on_push)();
    }

    /// Push a completion that returns a value, and return a [`Future`] that
    /// resolves with that value once the completion drains (on the next
    /// `flush()`). Used by async code running on a [`WorkerContext`]'s
    /// executor that needs to run boa-touching logic under `&mut Context`
    /// and hand the result back into the async flow — e.g. driving a JS
    /// generator from a spawned task (see `tur_launch`).
    ///
    /// The future resolves to `None` if the completion queue is dropped
    /// before the completion runs (shouldn't happen in normal operation —
    /// the queue lives for the app's lifetime).
    ///
    /// `T` must be `'static` (it crosses the drain boundary via a oneshot);
    /// single-threaded, so `!Send` values (e.g. `JsValue`) are fine.
    pub fn run<T: 'static>(&self, f: impl FnOnce(&mut Context) -> T + 'static) -> RunFuture<T> {
        let (tx, rx) = futures::channel::oneshot::channel();
        let completion: Completion = Box::new(move |ctx| {
            let v = f(ctx);
            // Send failure means the caller's future was dropped (e.g.
            // aborted) before the completion drained — harmless.
            let _ = tx.send(v);
            Ok(())
        });
        self.push(completion);
        RunFuture { rx }
    }
}

/// Future returned by [`CompletionHandle::run`]. Resolves to the
/// completion closure's return value (or `None` if the queue was dropped).
pub struct RunFuture<T> {
    rx: futures::channel::oneshot::Receiver<T>,
}

impl<T> Future for RunFuture<T> {
    type Output = Option<T>;
    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match Pin::new(&mut self.rx).poll(cx) {
            std::task::Poll::Ready(Ok(v)) => std::task::Poll::Ready(Some(v)),
            std::task::Poll::Ready(Err(_)) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl Clone for CompletionHandle {
    fn clone(&self) -> Self {
        Self {
            pending: self.pending.clone(),
            on_push: self.on_push.clone(),
        }
    }
}
