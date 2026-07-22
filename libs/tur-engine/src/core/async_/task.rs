//! Async task primitives exported by `tur:std`: `sleep` + `launch`.
//!
//! These replace the old `setTimeout`/`setInterval`/`clearTimeout`/
//! `clearInterval` globals. There is no timer-id registry and no separate
//! cancel handle — instead:
//!
//! - `sleep(ms): Promise<void>` resolves after `ms` (engine time), backed by
//!   [`AsyncExecutor::sleep`]. The frame scheduler already wakes precisely at
//!   the deadline via `AsyncExecutor::next_timer_delay`, so no extra wiring.
//! - `launch(gen): Task` runs a generator function as a cancellable coroutine.
//!   The generator `yield`s Promises (typically `sleep(ms)`); the driver
//!   resumes it when each yielded promise resolves. The returned `Task`
//!   exposes `cancel()`, which stops further resumption. Generators — unlike
//!   async functions — can be externally stepped/abandoned, which is what
//!   makes real cancellation possible.
//!
//! Rejection semantics: when a yielded promise rejects, the driver throws the
//! rejection reason into the generator at the `yield` point (via
//! `iterator.throw`), so a `try/catch` around the `yield` catches it — the
//! same ergonomics as `await`. If the generator body does not catch it, the
//! driver logs the uncaught rejection and stops resuming. This is safe to use
//! with fallible Promises (`clipboard.readText`, `http`, `fetch`), not just
//! `sleep`.
//!
//! Cancellation semantics: `cancel()` simply stops resuming. The in-flight
//! `sleep` Rust future still fires later and resolves its promise; the driver
//! ignores a cancelled task's resolution, so the generator body after the
//! current `yield` never runs again.
//!
//! Both fns are ctx-bound `Ptr`s (see `bound_native`): `args[0]` is the
//! `TurJsContext`, user args start at `args[1]`.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsPromise;
use boa_engine::object::{FunctionObjectBuilder, JsObject};
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};

use crate::core::js_runtime::helpers::{extract_ctx, FnEntry};

/// Bridge function table entries for `tur:std`: `sleep` + `launch`.
pub fn fns() -> Vec<FnEntry> {
    vec![("sleep", 1, tur_sleep as _), ("launch", 1, tur_launch as _)]
}

/// `sleep(ms): Promise<void>` — resolves after `ms` milliseconds (engine
/// time). Backed by [`AsyncExecutor::sleep`]; the engine's frame loop wakes at
/// the deadline via `next_timer_delay`.
fn tur_sleep(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let executor = js_ctx.async_executor().clone();
    let ms = args
        .get_or_undefined(1)
        .as_number()
        .unwrap_or(0.0)
        .max(0.0) as u64;

    let (promise, resolvers) = JsPromise::new_pending(ctx);
    let need_paint = js_ctx.need_paint.clone();
    let exec = executor.clone();
    executor.spawn_detached(async move {
        exec.sleep(Duration::from_millis(ms)).await;
        // Settle the promise under `&mut Context` on the next flush. Setting
        // `need_paint` mirrors the old timer's flush flag so a paint follows
        // even if the `.then` body makes no reactive `set`.
        exec.complete(Box::new(move |ctx| {
            need_paint.set(true);
            resolvers
                .resolve
                .call(&JsValue::undefined(), &[], ctx)?;
            Ok(())
        }));
    });
    Ok(promise.into())
}

/// `launch(gen): Task` — runs a zero-arg generator function as a cancellable
/// coroutine. The generator must `yield` Promises (e.g. `yield sleep(ms)`);
/// each resolved promise resumes the generator, passing the resolved value
/// back as the `yield` result. A rejected yielded promise throws its reason
/// into the generator at the `yield` (catchable with `try/catch`); if
/// uncaught, the driver logs and stops. Returns a `Task` with a `cancel()`
/// method.
///
/// Drives the iterator generically via `.next(value)` (the ES iterator
/// protocol), so it works with native `function*` generators AND
/// SWC/TypeScript-down-levelled generators (tslib `_ts_generator`), which is
/// what bundled code (e.g. the rspack-built playground `impl.js`) produces.
fn tur_launch(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let gen_fn = args
        .get_or_undefined(1)
        .as_object()
        .filter(|o| o.is_callable())
        .ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message("launch: expected a generator function"))
        })?;

    // Invoke the generator function to obtain the iterator object. For native
    // generators this is a `Generator`; for down-levelled (tslib) generators
    // it's a plain object with a `.next` method. We drive both the same way.
    let gen_value = gen_fn.call(&JsValue::undefined(), &[], ctx)?;
    let gen_obj = gen_value.as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("launch: generator function did not return an iterator"))
    })?;
    require_iterator(&gen_obj, ctx)?;

    let cancelled = Rc::new(Cell::new(false));

    // Kick off the first step synchronously. The generator runs until its
    // first `yield`, then parks on the yielded promise.
    let initial = build_step(gen_obj, cancelled.clone(), ctx)?;
    initial.call(&JsValue::undefined(), &[], ctx)?;

    // Build the `Task` handle: `{ cancel(): void }`.
    let cancelled_for_cancel = cancelled;
    let cancel_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            cancelled_for_cancel.set(true);
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

/// Confirm `obj` has a callable `.next` (i.e. is an iterator).
fn require_iterator(obj: &JsObject, ctx: &mut Context) -> JsResult<()> {
    let has_next = obj
        .get(js_string!("next"), ctx)?
        .as_object()
        .is_some_and(|o| o.is_callable());
    if !has_next {
        return Err(JsError::from(JsNativeError::typ()
            .with_message("launch: generator function did not return an iterator (no callable .next)")));
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
            JsError::from(JsNativeError::typ().with_message(
                "launch: iterator has no callable .throw to deliver rejection",
            ))
        })?;
    throw_fn.call(&iter.clone().into(), &[reason], ctx)
}

/// Shared post-resume logic: parse the iterator result `{done, value}`, and
/// if the generator is still alive, attach the next pair of `.then` handlers
/// (fulfilled + rejected) to the newly-yielded thenable so the coroutine
/// resumes — or surfaces a rejection — when it settles.
fn drive_result(
    result: JsValue,
    iterator: &JsObject,
    cancelled: Rc<Cell<bool>>,
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let Some(result_obj) = result.as_object() else {
        tracing::error!("launch: iterator did not return an object");
        return Ok(JsValue::undefined());
    };
    let done = result_obj.get(js_string!("done"), ctx)?.to_boolean();
    if done {
        return Ok(JsValue::undefined());
    }
    let value = result_obj.get(js_string!("value"), ctx)?;

    // The yielded value must be a thenable (a Promise such as `sleep(ms)`).
    // Attach both the fulfilled and rejected handlers to its `.then`.
    if let Some(obj) = value.as_object()
        && let Some(then_fn) = obj
            .get(js_string!("then"), ctx)?
            .as_object()
            .filter(|f| f.is_callable())
        {
            let on_fulfilled = build_step(iterator.clone(), cancelled.clone(), ctx)?;
            let on_rejected = build_reject_step(iterator.clone(), cancelled.clone(), ctx)?;
            then_fn.call(
                &obj.clone().into(),
                &[on_fulfilled.into(), on_rejected.into()],
                ctx,
            )?;
            return Ok(JsValue::undefined());
        }
    tracing::error!("launch: generator yielded a non-thenable; yield a Promise (e.g. sleep(ms))");
    Ok(JsValue::undefined())
}

/// Build the resume callback for one step of a coroutine. Invoked when the
/// previously-yielded promise resolves: passes the resolved value back into
/// the generator as the `yield` result, then drives the new result. A fresh
/// closure is built per resume (rather than reusing one self-referencing fn)
/// to sidestep Rust's recursive-closure limitation.
fn build_step(
    iterator: JsObject,
    cancelled: Rc<Cell<bool>>,
    ctx: &mut Context,
) -> JsResult<JsObject> {
    let iter_for_step = iterator.clone();
    let cancelled_for_step = cancelled.clone();
    let step_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            if cancelled_for_step.get() {
                return Ok(JsValue::undefined());
            }
            // `args[0]` is the resolved value of the previously-yielded
            // promise; feed it back into the generator as the `yield` result.
            let resume_value = args.get_or_undefined(0).clone();
            match iter_next(&iter_for_step, resume_value, ctx) {
                Ok(result) => drive_result(result, &iter_for_step, cancelled_for_step.clone(), ctx),
                Err(e) => {
                    tracing::error!("launch: generator threw: {e}");
                    Ok(JsValue::undefined())
                }
            }
        })
    };
    Ok(FunctionObjectBuilder::new(ctx.realm(), step_fn)
        .name(js_string!("launchStep"))
        .build()
        .into())
}

/// Build the rejection callback for one step of a coroutine. Invoked when the
/// previously-yielded promise rejects: throws the rejection reason into the
/// generator at the `yield` point (via `iterator.throw`), where it surfaces as
/// a thrown error catchable by a `try/catch` around the `yield`. If the body
/// does not catch it, `.throw()` rethrows and the driver logs + stops.
fn build_reject_step(
    iterator: JsObject,
    cancelled: Rc<Cell<bool>>,
    ctx: &mut Context,
) -> JsResult<JsObject> {
    let iter_for_step = iterator.clone();
    let cancelled_for_step = cancelled.clone();
    let step_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            if cancelled_for_step.get() {
                return Ok(JsValue::undefined());
            }
            // `args[0]` is the rejection reason; throw it into the generator.
            let reason = args.get_or_undefined(0).clone();
            match iter_throw(&iter_for_step, reason, ctx) {
                Ok(result) => drive_result(result, &iter_for_step, cancelled_for_step.clone(), ctx),
                Err(e) => {
                    tracing::error!("launch: generator threw (uncaught rejection): {e}");
                    Ok(JsValue::undefined())
                }
            }
        })
    };
    Ok(FunctionObjectBuilder::new(ctx.realm(), step_fn)
        .name(js_string!("launchRejectStep"))
        .build()
        .into())
}
