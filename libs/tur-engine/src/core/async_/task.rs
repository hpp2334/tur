//! Async task primitives exported by `tur:std`: `sleep` + `launch`.
//!
//! These replace the old `setTimeout`/`setInterval`/`clearTimeout`/
//! `clearInterval` globals. There is no timer-id registry and no separate
//! cancel handle — instead:
//!
//! - `sleep(ms): Promise<void>` resolves after `ms` (engine time), backed by
//!   [`crate::core::scheduler::WorkerContext::sleep`]. The worker-side
//!   scheduler provides a platform-specific `Sleep(BoxFuture)` (setTimeout
//!   on wasm, tokio::time::sleep on native, virtual clock on tests). When
//!   the Sleep resolves, the completion settle the promise + fires
//!   `on_push`, which self-sends `WorkerMsg::Wake` so the worker flushes
//!   promptly to drain.
//! - `launch(gen): Task` runs a generator function as a cancellable
//!   coroutine. The generator `yield`s Promises (typically `sleep(ms)`);
//!   each resolved promise resumes the generator, passing the resolved
//!   value back as the `yield` result. A rejected yielded promise throws
//!   its reason into the generator at the `yield` (catchable with
//!   `try/catch`); if uncaught, the driver logs and stops. Returns a `Task`
//!   with a `cancel()` method.
//!
//! ## Cancellation model
//!
//! `launch` runs the generator inside a task spawned via
//! [`WorkerContext::spawn_local`], which returns a [`TaskHandle`]. The
//! `Task.cancel()` JS method calls `TaskHandle::abort()`, which **drops the
//! driver future at its next `.await`** — so a pending `sleep` is dropped
//! (its timer cancelled) and the generator never resumes past the current
//! `yield`. This is real cancellation, not a soft flag.
//!
//! The driver is structured so every boa-touching op (calling the generator
//! function, `iter.next(v)`, `iter.throw(r)`, attaching `.then`) runs inside
//! a [`CompletionHandle::run`] closure (has `&mut Context`); awaiting a
//! yielded promise uses [`promise_to_future`], which attaches `.then` under
//! `Context` and then polls a shared slot from the async task.
//!
//! ## Generators vs. async functions
//!
//! Driven via `.next(value)` / `.throw(reason)` (the ES iterator protocol),
//! so it works with native `function*` generators AND SWC/TypeScript
//! down-levelled generators (tslib `_ts_generator`), which is what bundled
//! code (e.g. the rspack-built playground `impl.js`) produces.
//!
//! Both fns are ctx-bound `Ptr`s (see `bound_native`): `args[0]` is the
//! `TurInstanceContext`, user args start at `args[1]`.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Poll, Waker};

use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsPromise;
use boa_engine::object::{FunctionObjectBuilder, JsObject};
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};

use crate::core::js_runtime::helpers::{FnEntry, extract_js_ctx};
use crate::core::scheduler::TaskHandle;

/// Bridge function table entries for `tur:std`: `sleep` + `launch`.
pub fn fns() -> Vec<FnEntry> {
    vec![("sleep", 1, tur_sleep as _), ("launch", 1, tur_launch as _)]
}

/// `sleep(ms): Promise<void>` — resolves after `ms` milliseconds (engine
/// time). Backed by [`crate::core::scheduler::WorkerContext::sleep`]; the
/// completion self-sends `WorkerMsg::Wake` so the worker flushes promptly.
fn tur_sleep(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let worker_ctx = js_ctx.worker_ctx().clone();
    let completion_handle = js_ctx.completion_handle();
    let flush_tasks = js_ctx.flush_task_handle();
    let ms = args.get_or_undefined(1).as_number().unwrap_or(0.0).max(0.0) as u64;

    let (promise, resolvers) = JsPromise::new_pending(ctx);
    // The completion runs in-flush (drained at the top of the flush loop),
    // so `request_paint` is the flag-only fast path there — it sets the
    // paint flag this same flush reads.
    let js_ctx_for_completion = js_ctx.clone();
    let worker_ctx_for_loop = worker_ctx.clone();
    let fut: Pin<Box<dyn std::future::Future<Output = ()> + 'static>> = Box::pin(async move {
        worker_ctx_for_loop
            .sleep(std::time::Duration::from_millis(ms))
            .await;
        // Settle the promise under `&mut Context` on the next flush.
        // `request_paint` mirrors the old timer's flush flag so a paint
        // follows even if the `.then` body makes no reactive `set`.
        completion_handle.push(Box::new(move |ctx| {
            js_ctx_for_completion.request_paint();
            resolvers.resolve.call(&JsValue::undefined(), &[], ctx)?;
            Ok(())
        }));
    });
    // Push to the flush-driven queue (not `worker_ctx.spawn_local`) so the
    // sleep future is polled inside `flush()` — letting a clock `advance`
    // that reaches the deadline resolve the sleep *within the same frame*
    // rather than lagging to the next. See `core::async_::flush_tasks`.
    flush_tasks.spawn(fut);
    Ok(promise.into())
}

/// `launch(gen): Task` — runs a zero-arg generator function as a cancellable
/// coroutine. See the module docs for the full model.
fn tur_launch(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let completion_handle = js_ctx.completion_handle();
    let flush_tasks = js_ctx.flush_task_handle();

    let gen_fn = args
        .get_or_undefined(1)
        .as_object()
        .filter(|o| o.is_callable())
        .ok_or_else(|| {
            JsError::from(
                JsNativeError::typ().with_message("launch: expected a generator function"),
            )
        })?;

    // Drive the generator inside a flush-driven, abortable task (see
    // `core::async_::flush_tasks`). Returns a `TaskHandle` whose `abort()`
    // becomes the JS `Task.cancel()` — real cancellation, not a soft flag.
    let handle: TaskHandle = flush_tasks.spawn(Box::pin(async move {
        // Construct the iterator under Context.
        let iter = match completion_handle
            .run(move |ctx| {
                let gen_value = gen_fn.call(&JsValue::undefined(), &[], ctx)?;
                let obj = gen_value.as_object().ok_or_else(|| {
                    JsError::from(
                        JsNativeError::typ()
                            .with_message("launch: generator function did not return an iterator"),
                    )
                })?;
                require_iterator(&obj, ctx)?;
                Ok::<_, JsError>(obj)
            })
            .await
        {
            Some(Ok(iter)) => iter,
            Some(Err(e)) => {
                tracing::error!("launch: {e}");
                return;
            }
            None => return,
        };

        // Resume value threaded between steps: `Ok(v)` → next(v),
        // `Err(r)` → throw(r).
        let mut resume: StepResume = StepResume::Next(JsValue::undefined());
        loop {
            let iter_for_step = iter.clone();
            let resume_for_step =
                std::mem::replace(&mut resume, StepResume::Next(JsValue::undefined()));
            // Drive one iterator step under Context (next or throw), then
            // attach `.then` to the yielded thenable.
            let step_result = completion_handle
                .run(move |ctx| step_under_ctx(ctx, &iter_for_step, resume_for_step))
                .await;
            let pf = match step_result {
                None => return,
                Some(Err(e)) => {
                    tracing::error!("launch: generator threw: {e}");
                    return;
                }
                Some(Ok(Step::Done)) => return,
                Some(Ok(Step::Pending(pf))) => pf,
            };
            // Await the yielded promise outside Context.
            match pf.await {
                Ok(v) => resume = StepResume::Next(v),
                Err(reason) => resume = StepResume::Throw(reason),
            }
        }
    }));

    // Build the `Task` handle: `{ cancel(): void }`. The TaskHandle is
    // shared (Rc) between this scope + the cancel closure; abort(&self)
    // needs only &self, so the Fn closure can fire it any number of times.
    let handle = Rc::new(handle);
    let handle_for_cancel = handle.clone();
    let cancel_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            handle_for_cancel.abort();
            Ok(JsValue::undefined())
        })
    };
    let cancel_obj = FunctionObjectBuilder::new(ctx.realm(), cancel_fn)
        .name(js_string!("cancel"))
        .build();
    let proto = ctx.intrinsics().constructors().object().prototype();
    let task = JsObject::from_proto_and_data(proto, ());
    task.create_data_property(js_string!("cancel"), JsValue::from(cancel_obj), ctx)?;
    Ok(task.into())
}

/// Resume directive for the next iterator step.
enum StepResume {
    /// `iter.next(value)` — feed the previously-yielded promise's resolved
    /// value back into the generator.
    Next(JsValue),
    /// `iter.throw(reason)` — deliver a rejection at the `yield` point.
    Throw(JsValue),
}

/// Outcome of one iterator step.
enum Step {
    /// The generator returned `{ done: true }` — finished.
    Done,
    /// Still alive; await this thenable before the next step.
    Pending(PromiseFuture),
}

/// Run one iterator step (`next` or `throw`) under `&mut Context`, parse the
/// result, and (if the generator is still alive) attach `.then` to the
/// yielded value. Returns the [`Step`] to drive next.
fn step_under_ctx(ctx: &mut Context, iter: &JsObject, resume: StepResume) -> JsResult<Step> {
    let result = match resume {
        StepResume::Next(v) => iter_next(iter, v, ctx)?,
        StepResume::Throw(r) => iter_throw(iter, r, ctx)?,
    };
    let result_obj = result.as_object().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("launch: iterator did not return an object"),
        )
    })?;
    let done = result_obj.get(js_string!("done"), ctx)?.to_boolean();
    if done {
        return Ok(Step::Done);
    }
    let value = result_obj.get(js_string!("value"), ctx)?;
    let pf = promise_to_future(value, ctx)?;
    Ok(Step::Pending(pf))
}

/// Confirm `obj` has a callable `.next` (i.e. is an iterator).
fn require_iterator(obj: &JsObject, ctx: &mut Context) -> JsResult<()> {
    let has_next = obj
        .get(js_string!("next"), ctx)?
        .as_object()
        .is_some_and(|o| o.is_callable());
    if !has_next {
        return Err(JsError::from(JsNativeError::typ().with_message(
            "launch: generator function did not return an iterator (no callable .next)",
        )));
    }
    Ok(())
}

/// Call `iterator.next(value)` → `{done, value}`.
fn iter_next(iter: &JsObject, value: JsValue, ctx: &mut Context) -> JsResult<JsValue> {
    let next_fn = iter
        .get(js_string!("next"), ctx)?
        .as_object()
        .ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message("launch: iterator lost its .next"))
        })?;
    next_fn.call(&iter.clone().into(), &[value], ctx)
}

/// Call `iterator.throw(reason)` → `{done, value}`. Delivers a yielded
/// promise's rejection to the generator at the `yield` point, surfacing it as
/// a thrown error that the generator body may `catch`. Returns `Err` if the
/// iterator has no callable `.throw` (it cannot accept the throw) or if the
/// generator body did not catch the throw (the throw propagates back out of
/// `.throw`).
fn iter_throw(iter: &JsObject, reason: JsValue, ctx: &mut Context) -> JsResult<JsValue> {
    let throw_fn = iter
        .get(js_string!("throw"), ctx)?
        .as_object()
        .filter(|o| o.is_callable())
        .ok_or_else(|| {
            JsError::from(
                JsNativeError::typ()
                    .with_message("launch: iterator has no callable .throw to deliver rejection"),
            )
        })?;
    throw_fn.call(&iter.clone().into(), &[reason], ctx)
}

// ── Promise → Future bridge ──────────────────────────────────────────────

/// A thenable (object with a callable `.then`) turned into a Rust future.
///
/// Built by [`promise_to_future`] under `&mut Context` (it attaches `.then`
/// then). Polling stores the [`Waker`]; when the promise settles, the
/// `.then` reaction fills the shared slot and wakes the task.
///
/// Single-threaded (holds `Rc`); fine for `spawn_local` tasks.
struct PromiseFuture {
    /// `Ok(value)` on fulfill, `Err(reason)` on reject. Filled by the
    /// `.then` reaction; taken by `poll`.
    slot: Rc<RefCell<Option<Result<JsValue, JsValue>>>>,
    /// Waker registered on first `poll`; used by the `.then` reaction to
    /// re-poll when the promise settles later (e.g. after `sleep(ms)`).
    waker: Rc<RefCell<Option<Waker>>>,
}

impl Future for PromiseFuture {
    type Output = Result<JsValue, JsValue>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        // Register/refresh the waker before checking the slot: if the
        // promise settles after this poll, the reaction can wake us.
        *self.waker.borrow_mut() = Some(cx.waker().clone());
        match self.slot.borrow_mut().take() {
            Some(v) => Poll::Ready(v),
            None => Poll::Pending,
        }
    }
}

/// Attach `.then(on_fulfilled, on_rejected)` to a yielded thenable and
/// return a [`PromiseFuture`] that resolves when it settles. The thenable
/// must be a JS object with a callable `.then` (a `JsPromise` or any
/// promise-like value — works with SWC/tslib-down-levelled code too).
///
/// The `.then` reactions run under `&mut Context` during boa's microtask
/// drain (inside `flush()`); they fill the shared slot + wake the polling
/// task. If the promise was already settled, the reaction runs on the next
/// boa drain — but the [`CompletionHandle::run`] completion that built this
/// future has already returned, and its oneshot re-polls the task, so the
/// pre-filled slot is read on that same cycle. No set-waker race.
fn promise_to_future(thenable: JsValue, ctx: &mut Context) -> JsResult<PromiseFuture> {
    let thenable_obj = thenable.as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(
            "launch: generator yielded a non-thenable; yield a Promise (e.g. sleep(ms))",
        ))
    })?;
    let then_fn = thenable_obj
        .get(js_string!("then"), ctx)?
        .as_object()
        .filter(|f| f.is_callable())
        .ok_or_else(|| {
            JsError::from(
                JsNativeError::typ().with_message("launch: yielded value has no callable .then"),
            )
        })?;

    let slot: Rc<RefCell<Option<Result<JsValue, JsValue>>>> = Rc::new(RefCell::new(None));
    let waker: Rc<RefCell<Option<Waker>>> = Rc::new(RefCell::new(None));

    let slot_f = slot.clone();
    let waker_f = waker.clone();
    let on_fulfilled = unsafe {
        NativeFunction::from_closure(move |_this, args, _ctx| {
            let v = args.get_or_undefined(0).clone();
            // Guard against a double-settle (shouldn't happen, but cheap).
            if slot_f.borrow_mut().replace(Ok(v)).is_none()
                && let Some(w) = waker_f.borrow().as_ref()
            {
                w.wake_by_ref();
            }
            Ok(JsValue::undefined())
        })
    };
    let slot_r = slot.clone();
    let waker_r = waker.clone();
    let on_rejected = unsafe {
        NativeFunction::from_closure(move |_this, args, _ctx| {
            let r = args.get_or_undefined(0).clone();
            if slot_r.borrow_mut().replace(Err(r)).is_none()
                && let Some(w) = waker_r.borrow().as_ref()
            {
                w.wake_by_ref();
            }
            Ok(JsValue::undefined())
        })
    };

    let on_fulfilled_obj = FunctionObjectBuilder::new(ctx.realm(), on_fulfilled)
        .name(js_string!("launchThenFulfilled"))
        .build();
    let on_rejected_obj = FunctionObjectBuilder::new(ctx.realm(), on_rejected)
        .name(js_string!("launchThenRejected"))
        .build();
    then_fn.call(
        &thenable_obj.clone().into(),
        &[on_fulfilled_obj.into(), on_rejected_obj.into()],
        ctx,
    )?;

    Ok(PromiseFuture { slot, waker })
}
