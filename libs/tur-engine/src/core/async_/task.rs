//! Async task primitives exported by `tur:std`: `sleep` + the shared
//! `Task<T> = { promise, cancel() }` handle every async engine API returns.
//!
//! These replace the old `setTimeout`/`setInterval`/`clearTimeout`/
//! `clearInterval` globals — and the former `launch` generator driver.
//! Async composition is plain `async`/`await` + `.then` (boa runs native
//! async functions; no engine-side driver is needed), and cancellation is
//! per-operation via the returned handle:
//!
//! - `sleep(ms): Task<void>` resolves after `ms` (engine time), backed by
//!   [`crate::core::scheduler::WorkerContext::sleep`]. The worker-side
//!   scheduler provides a platform-specific `Sleep(BoxFuture)` (setTimeout
//!   on wasm, tokio::time::sleep on native, virtual clock on tests). When
//!   the Sleep resolves, the completion settles the promise + fires
//!   `on_push`, which self-sends `WorkerMsg::Wake` so the worker flushes
//!   promptly to drain.
//! - Every other async bridge (`clipboard.readText`/`writeText`, `request`,
//!   `requestStream`, `filePicker.pick`/`saveFile`) returns the same
//!   [`make_task`] handle.
//!
//! ## Cancellation model
//!
//! `task.cancel()`:
//!
//! 1. runs the op's `extra_cancel` hook (engine-side cleanup — e.g.
//!    `requestStream` wire-aborts its response stream),
//! 2. aborts the op's [`TaskHandle`] (the driver future is dropped at its
//!    next poll point — a pending `sleep` timer is really cleared, an
//!    unpolled HTTP request is never sent, an in-flight one is discarded),
//! 3. **rejects `task.promise` with a `CancelError`**
//!    (`e.name === "CancelError"`; see [`is_cancel_error`]).
//!
//! `cancel()` is idempotent, and a no-op for an already-settled promise
//! (promise settle-once semantics make the late reject harmless — though
//! op-specific abort still runs, e.g. cancelling a stream mid-consumption).
//! One-shot host ops (a clipboard read already dispatched to the host
//! thread) may still complete underneath; their result is discarded.
//!
//! Debounce idiom (the no-op rejection handler *is* the cancelled branch):
//!
//! ```js
//! t?.cancel(); t = sleep(300);
//! t.promise.then(show, () => {});
//! ```
//!
//! Both fns are ctx-bound `Ptr`s (see `bound_native`): `args[0]` is the
//! `TurInstanceContext`, user args start at `args[1]`.

use std::pin::Pin;
use std::rc::Rc;

use boa_engine::builtins::promise::ResolvingFunctions;
use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsPromise;
use boa_engine::object::{FunctionObjectBuilder, JsObject};
use boa_engine::{Context, JsArgs, JsNativeError, JsResult, JsValue};

use crate::core::js_runtime::helpers::{FnEntry, extract_js_ctx};
use crate::core::scheduler::TaskHandle;

/// `name` property of the rejection reason produced by `Task.cancel()`.
pub const CANCEL_ERROR_NAME: &str = "CancelError";

/// Bridge function table entries for `tur:std`: `sleep` + `isCancelError`.
pub fn fns() -> Vec<FnEntry> {
    vec![
        ("sleep", 1, tur_sleep as _),
        ("isCancelError", 1, tur_is_cancel_error as _),
    ]
}

/// `sleep(ms): Task<void>` — resolves after `ms` milliseconds (engine
/// time). Backed by [`crate::core::scheduler::WorkerContext::sleep`]; the
/// completion self-sends `WorkerMsg::Wake` so the worker flushes promptly.
/// `cancel()` aborts the timer (it never fires) and rejects with a
/// [`CancelError`](CANCEL_ERROR_NAME).
fn tur_sleep(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let worker_ctx = js_ctx.worker_ctx().clone();
    let completion_handle = js_ctx.completion_handle();
    let flush_tasks = js_ctx.flush_task_handle();
    let ms = args.get_or_undefined(1).as_number().unwrap_or(0.0).max(0.0) as u64;

    let (promise, resolvers) = JsPromise::new_pending(ctx);
    let resolvers_for_task = resolvers.clone();
    // The completion runs in-flush (drained at the top of the flush loop),
    // so `request_frame` is the fast path there — it sets the
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
            js_ctx_for_completion.request_frame();
            resolvers.resolve.call(&JsValue::undefined(), &[], ctx)?;
            Ok(())
        }));
    });
    // Push to the flush-driven queue (not `worker_ctx.spawn_local`) so the
    // sleep future is polled inside `flush()` — letting a clock `advance`
    // that reaches the deadline resolve the sleep *within the same frame*
    // rather than lagging to the next. See `core::async_::flush_tasks`.
    // The returned handle makes `Task.cancel()` a real timer abort (the
    // tracked future is dropped → the completion above never pushes).
    let handle: TaskHandle = flush_tasks.spawn(fut);
    Ok(make_task(ctx, &promise, &resolvers_for_task, Some(handle), None).into())
}

/// `isCancelError(reason): boolean` — true when `reason` is the rejection
/// produced by `Task.cancel()` (an error whose `name` is
/// [`CANCEL_ERROR_NAME`]).
fn tur_is_cancel_error(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let v = args.get_or_undefined(1);
    let is = v
        .as_object()
        .map(|o| {
            o.get(js_string!("name"), ctx)
                .map(|n| n == JsValue::from(js_string!(CANCEL_ERROR_NAME)))
                .unwrap_or(false)
        })
        .unwrap_or(false);
    Ok(JsValue::from(is))
}

/// Build the `CancelError` rejection reason: an `Error` whose `name` is
/// [`CANCEL_ERROR_NAME`]. Shared by every `Task.cancel()` so the catch
/// branch can test `e.name` (or [`tur_is_cancel_error`]).
pub fn cancel_error(ctx: &mut Context) -> JsValue {
    let err = JsNativeError::error()
        .with_message("cancelled")
        .into_opaque(ctx);
    let _ = err.create_data_property_or_throw(
        js_string!("name"),
        JsValue::from(js_string!(CANCEL_ERROR_NAME)),
        ctx,
    );
    err.into()
}

/// Build the shared `Task<T>` JS handle: `{ promise, cancel() }`.
///
/// Every async bridge API returns this object. `cancel()`:
/// 1. runs `extra_cancel` (engine-side cleanup under no JS access — e.g.
///    `requestStream` drops its response pipe so pending pulls resolve
///    `{done: true}`),
/// 2. aborts `abort` (drops the op's driver future at its next poll),
/// 3. rejects `promise` with a [`CancelError`](cancel_error).
///
/// Promise settle-once semantics make double cancels and post-settlement
/// cancels harmless (the reject is a silent no-op; the abort still runs).
///
/// The `cancel` closure is one `unsafe NativeFunction::from_closure` —
/// the established pattern (the former `launch` handle used it): captures
/// are `Rc`/handle state plus the reject `JsFunction`, whose boa handle is
/// GC-rooted by clone.
pub fn make_task(
    ctx: &mut Context,
    promise: &JsPromise,
    resolvers: &ResolvingFunctions,
    abort: Option<TaskHandle>,
    extra_cancel: Option<Box<dyn Fn() + 'static>>,
) -> JsObject {
    let reject = resolvers.reject.clone();
    let abort = Rc::new(abort);
    let cancel_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx| {
            if let Some(extra) = &extra_cancel {
                extra();
            }
            if let Some(handle) = abort.as_ref() {
                handle.abort();
            }
            let reason = cancel_error(ctx);
            reject.call(&JsValue::undefined(), &[reason], ctx)?;
            Ok(JsValue::undefined())
        })
    };
    let cancel_obj = FunctionObjectBuilder::new(ctx.realm(), cancel_fn)
        .length(0)
        .name(js_string!("cancel"))
        .build();

    let task =
        JsObject::from_proto_and_data(ctx.intrinsics().constructors().object().prototype(), ());
    let _ = task.create_data_property(js_string!("promise"), promise.clone(), ctx);
    let _ = task.create_data_property(js_string!("cancel"), JsValue::from(cancel_obj), ctx);
    task
}
