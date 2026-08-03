//! Engine-side completion queue — the boa-bridge half of the old `AsyncExecutor`.
//!
//! Spawned futures (clipboard reads, http requests, file-picker dialogs,
//! caret blink tasks) need to settle `JsPromise`s under `&mut Context`.
//! The completion queue carries closures from the future's completion
//! (running on a `WorkerScheduler`'s executor) to the engine's `flush()`
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
//! (`WorkerScheduler::spawn_local` → embedder's executor); only the
//! completion queue remains, narrow and focused.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use boa_engine::Context;
use boa_engine::JsResult;

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
    on_push: Rc<dyn Fn()>,
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
    /// promptly whenever a future completes.
    pub fn new(on_push: impl Fn() + 'static) -> Self {
        Self {
            pending: Rc::new(RefCell::new(VecDeque::new())),
            on_push: Rc::new(on_push),
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
    on_push: Rc<dyn Fn()>,
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
}

impl Clone for CompletionHandle {
    fn clone(&self) -> Self {
        Self {
            pending: self.pending.clone(),
            on_push: self.on_push.clone(),
        }
    }
}
