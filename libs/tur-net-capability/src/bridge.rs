//! `tur:net` HTTP bridge: `request(opts)` + `requestStream(opts)`, both
//! returning the shared `Task` handle (`{ promise, cancel() }`).
//!
//! Mirrors the clipboard bridge pattern in tur-engine: a **ctx-bound fn
//! pointer** (no captures) that reads its `Rc<dyn Http>` + scheduler
//! primitives from `TurInstanceContext`. The fn creates a pending
//! `JsPromise`, spawns a future via the instance context's `spawn_local`
//! that calls `Http::request(opts).await`, and returns
//! [`make_task`](tur_engine::core::async_::make_task)'s handle —
//! `task.cancel()` aborts the spawn (an unpollled request is never sent; an
//! in-flight one is discarded) and rejects with a `CancelError`.
//!
//! For `requestStream`, `cancel()` additionally **wire-aborts the stream**
//! (drops the response pipe — native: the producer's receiver is dropped →
//! the connection closes) so pending and subsequent `body.next()` pulls
//! resolve `{ done: true }` and `for await` loops exit cleanly.
//!
//! This file contains **no `unsafe`** — uses `NativeFunction::from_fn_ptr`
//! via the engine's `bound_native` helper instead of the previous
//! `unsafe NativeFunction::from_closure`. Captures are eliminated because
//! the needed state lives in the capability registry (populated by
//! [`crate::TurNetPlugin`] during `register`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::object::FunctionObjectBuilder;
use boa_engine::object::JsObject;
use boa_engine::object::builtins::{JsArrayBuffer, JsPromise, JsUint8Array};
use boa_engine::property::PropertyKey;
use boa_engine::{
    Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsSymbol, JsValue, NativeFunction,
    js_string,
};
use boa_gc::{Finalize, Trace};
use futures::StreamExt;
use futures::stream::LocalBoxStream;

use tur_engine::core::async_::{CompletionHandle, make_task};
use tur_engine::core::js_runtime::TurInstanceContext;
use tur_engine::core::js_runtime::helpers::{FnEntry, Ptr, extract_js_ctx};

/// Shorthand for the boxed byte-chunk stream used by the streaming bridge.
type ByteChunkStream = LocalBoxStream<'static, Result<Vec<u8>, String>>;

/// Shared stream state — `RefCell<Option<…>>` so `next()` can take the stream
/// out, poll one chunk, and put it back.
type SharedStream = Rc<RefCell<Option<ByteChunkStream>>>;

/// Shared boolean flag consulted from completion closures that run after the
/// native call returns (`Rc<Cell<bool>>` — cheap, JS-side only).
type SharedFlag = Rc<Cell<bool>>;

/// Valid range (bytes) for the JS `bufferBytes` option, enforced at parse
/// time so every backend sees uniform validation errors. The lower bound
/// keeps the pipe progressing (one byte at a time is legal, just slow); the
/// upper bound is a typo guard, not a hard limit on in-flight data by
/// intent — callers genuinely wanting looser backpressure can raise it.
const MIN_STREAM_BUFFER_BYTES: f64 = 1.0;
/// 64 MiB typo guard for `bufferBytes`.
const MAX_STREAM_BUFFER_BYTES: f64 = 64.0 * 1024.0 * 1024.0;

use crate::{Http, HttpBody, HttpOutcome, RequestOpts, ResponseType};
/// Bridge function tables entries for `tur:net`.
///
/// Returns `("request", …)` + `("requestStream", …)` — ctx-bound fn pointers
/// that read their `Http` + scheduler from `TurInstanceContext`. Both return
/// the shared `Task` (`{ promise, cancel() }`) handle.
pub fn fns() -> Vec<FnEntry> {
    vec![
        ("request", 1, tur_net_request as Ptr),
        ("requestStream", 1, tur_net_request_stream as Ptr),
    ]
}

/// `request(opts): Task<ResponseResult>` — performs an HTTP request via
/// the injected `Http` backend. `promise` rejects with `{ message }` on
/// network error or when no backend is registered; `cancel()` aborts the
/// request and rejects with a `CancelError`.
fn tur_net_request(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let http = js_ctx
        .capability()
        .of::<Http>()
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message("no http capability")))?
        .backend()
        .clone();
    let completion_handle = js_ctx.completion_handle();

    let (promise, resolvers) = JsPromise::new_pending(ctx);
    let resolvers_for_task = resolvers.clone();

    // Parse opts from the JS `{ url, method?, ... }` object. Note: args[0]
    // is the bound ctx_value (prepended by `bound_native`); the user's opts
    // arg is at index 1.
    let opts = match parse_request_opts(args, ctx) {
        Ok(o) => o,
        Err(msg) => {
            let e = JsObject::with_object_proto(ctx.intrinsics());
            let _ = e.create_data_property(
                js_string!("message"),
                JsValue::from(js_string!(msg.as_str())),
                ctx,
            );
            let _ = resolvers
                .reject
                .call(&JsValue::undefined(), &[e.into()], ctx);
            return Ok(make_task(ctx, &promise, &resolvers_for_task, None, None).into());
        }
    };

    let handle = js_ctx.spawn_local(|_aw| async move {
        let outcome = http.request(opts).await;
        completion_handle.push(Box::new(move |ctx| {
            resolve_outcome(&outcome, &resolvers, ctx)?;
            Ok(())
        }));
    });
    Ok(make_task(ctx, &promise, &resolvers_for_task, Some(handle), None).into())
}

fn parse_request_opts(
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> Result<RequestOpts, String> {
    // args[0] is the ctx_value; user opts is at index 1.
    let opts = args.get_or_undefined(1);
    let obj = opts.as_object().ok_or("request: options object required")?;

    let url = js_opt_str(&obj, "url", ctx).unwrap_or_default();
    let method = js_opt_str(&obj, "method", ctx).unwrap_or_else(|| "GET".to_string());
    let response_type_str =
        js_opt_str(&obj, "responseType", ctx).unwrap_or_else(|| "text".to_string());
    let response_type = if response_type_str == "bytes" {
        ResponseType::Bytes
    } else {
        ResponseType::Text
    };
    let username = js_opt_str(&obj, "username", ctx);
    let password = js_opt_str(&obj, "password", ctx);
    let stream_buffer_bytes = parse_buffer_bytes(&obj, ctx)?;

    let mut headers: Vec<(String, String)> = Vec::new();
    if let Some(hobj) = obj
        .get(js_string!("headers"), ctx)
        .map_err(|e| e.to_string())?
        .as_object()
    {
        let keys = hobj.own_property_keys(ctx).map_err(|e| e.to_string())?;
        for key in keys {
            let kstr = match &key {
                PropertyKey::String(s) => s.to_std_string_escaped(),
                PropertyKey::Index(i) => i.get().to_string(),
                PropertyKey::Symbol(_) => continue,
            };
            if let Ok(v) = hobj.get(key, ctx) {
                let vstr = v
                    .as_string()
                    .map(|s| s.to_std_string_escaped())
                    .unwrap_or_default();
                headers.push((kstr, vstr));
            }
        }
    }

    let body: Option<HttpBody> = match obj.get(js_string!("body"), ctx) {
        Ok(v) => {
            if let Some(s) = v.as_string() {
                Some(HttpBody::Text(s.to_std_string_escaped()))
            } else if let Some(o) = v.as_object() {
                JsArrayBuffer::from_object(o.clone())
                    .ok()
                    .and_then(|ab| ab.to_vec())
                    .map(HttpBody::Bytes)
            } else {
                None
            }
        }
        Err(_) => None,
    };

    Ok(RequestOpts {
        url,
        method,
        headers,
        body,
        response_type,
        username,
        password,
        stream_buffer_bytes,
    })
}

/// Parse + validate the optional `bufferBytes` option (streaming only).
/// `undefined`/`null`/absent → `None` (backend default); anything else must
/// be a finite integer in `1..=64 MiB`.
fn parse_buffer_bytes(
    obj: &JsObject,
    ctx: &mut boa_engine::Context,
) -> Result<Option<u32>, String> {
    let v = obj
        .get(js_string!("bufferBytes"), ctx)
        .map_err(|e| e.to_string())?;
    if v.is_undefined() || v.is_null() {
        return Ok(None);
    }
    let n = v
        .as_number()
        .ok_or_else(|| "bufferBytes must be a number".to_string())?;
    if !n.is_finite() {
        return Err("bufferBytes must be a finite number".to_string());
    }
    if n.fract() != 0.0 {
        return Err("bufferBytes must be an integer".to_string());
    }
    if n < MIN_STREAM_BUFFER_BYTES {
        return Err("bufferBytes must be >= 1".to_string());
    }
    if n > MAX_STREAM_BUFFER_BYTES {
        return Err(format!(
            "bufferBytes must be <= {MAX_STREAM_BUFFER_BYTES} (64 MiB)"
        ));
    }
    Ok(Some(n as u32))
}

fn js_opt_str(obj: &JsObject, key: &str, ctx: &mut boa_engine::Context) -> Option<String> {
    obj.get(js_string!(key), ctx)
        .ok()
        .and_then(|v| v.as_string().map(|s| s.to_std_string_escaped()))
}

/// Build the JS response object from `HttpOutcome` and resolve/reject the
/// pending promise. Mirrors the old frame-loop drain code from tur-wasm.
fn resolve_outcome(
    outcome: &HttpOutcome,
    resolvers: &boa_engine::builtins::promise::ResolvingFunctions,
    ctx: &mut boa_engine::Context,
) -> JsResult<()> {
    match outcome {
        HttpOutcome::Ok {
            status,
            status_text,
            headers,
            body,
        } => {
            let o = JsObject::with_object_proto(ctx.intrinsics());
            let _ = o.create_data_property(js_string!("ok"), JsValue::from(true), ctx);
            let _ =
                o.create_data_property(js_string!("status"), JsValue::from(*status as f64), ctx);
            let _ = o.create_data_property(
                js_string!("statusText"),
                JsValue::from(js_string!(status_text.as_str())),
                ctx,
            );
            let hmap = JsObject::with_object_proto(ctx.intrinsics());
            for (k, v) in headers {
                let _ = hmap.create_data_property(
                    js_string!(k.as_str()),
                    JsValue::from(js_string!(v.as_str())),
                    ctx,
                );
            }
            let _ = o.create_data_property(js_string!("headers"), JsValue::from(hmap), ctx);
            match body {
                HttpBody::Text(t) => {
                    let _ = o.create_data_property(
                        js_string!("bodyText"),
                        JsValue::from(js_string!(t.as_str())),
                        ctx,
                    );
                }
                HttpBody::Bytes(b) => {
                    use boa_engine::object::builtins::AlignedVec;
                    if let Ok(ab) =
                        JsArrayBuffer::from_byte_block(AlignedVec::from_iter(0, b.clone()), ctx)
                    {
                        let _ =
                            o.create_data_property(js_string!("bodyBytes"), JsValue::from(ab), ctx);
                    }
                }
            }
            resolvers
                .resolve
                .call(&JsValue::undefined(), &[o.into()], ctx)?;
        }
        HttpOutcome::Err(msg) => {
            let e = JsObject::with_object_proto(ctx.intrinsics());
            let _ = e.create_data_property(
                js_string!("message"),
                JsValue::from(js_string!(msg.as_str())),
                ctx,
            );
            resolvers
                .reject
                .call(&JsValue::undefined(), &[e.into()], ctx)?;
        }
    }
    Ok(())
}

// ===========================================================================
// Streaming: requestStream + async-iterable body
// ===========================================================================

/// Internal state for a streaming body object. Stored as `JsData` on the JS
/// body object so `next()` can access it via `this.downcast_ref::<StreamHandle>()`.
#[derive(Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
struct StreamHandle {
    stream: SharedStream,
    js_ctx: TurInstanceContext,
    completion_handle: CompletionHandle,
    /// Set by the owning Task's `cancel()` — the pipe was dropped; every
    /// `next()` resolves `{ done: true }` (ReadableStream cancel semantics).
    cancelled: SharedFlag,
    /// The pull protocol is serial: at most one `next()` in flight. A
    /// concurrent call rejects instead of queueing (queueing would silently
    /// build an unbounded promise chain — anti-backpressure).
    next_in_flight: SharedFlag,
}

/// Resolve a stream promise with `{ done: true }`.
fn resolve_stream_done(
    resolvers: &boa_engine::builtins::promise::ResolvingFunctions,
    ctx: &mut Context,
) -> JsResult<()> {
    let result = JsObject::with_object_proto(ctx.intrinsics());
    let _ = result.create_data_property(js_string!("done"), JsValue::from(true), ctx);
    resolvers
        .resolve
        .call(&JsValue::undefined(), &[result.into()], ctx)?;
    Ok(())
}

/// Reject a stream promise with `{ message }`.
fn reject_stream_error(
    resolvers: &boa_engine::builtins::promise::ResolvingFunctions,
    message: &str,
    ctx: &mut Context,
) -> JsResult<()> {
    let err_obj = JsObject::with_object_proto(ctx.intrinsics());
    let _ = err_obj.create_data_property(
        js_string!("message"),
        JsValue::from(js_string!(message)),
        ctx,
    );
    resolvers
        .reject
        .call(&JsValue::undefined(), &[err_obj.into()], ctx)?;
    Ok(())
}

/// `requestStream(opts): Task<StreamResponse>` — performs a streaming HTTP
/// request. The resolved value has `{ ok, status, statusText, headers, body }`
/// where `body` is an async iterable yielding `Uint8Array` chunks.
///
/// `task.cancel()` wire-aborts the download: it drops the response pipe
/// (native: the producer's receiver is dropped → the connection closes),
/// aborts the driver spawn, and — if the response promise hasn't settled
/// yet — rejects it with a `CancelError`. Once the response HAS settled,
/// cancelling is abort-only: pending and subsequent `body.next()` pulls
/// resolve `{ done: true }` so `for await` loops exit cleanly.
fn tur_net_request_stream(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let http = js_ctx
        .capability()
        .of::<Http>()
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message("no http capability")))?
        .backend()
        .clone();
    let worker_sched = js_ctx.clone();
    let completion_handle = js_ctx.completion_handle();

    let (promise, resolvers) = JsPromise::new_pending(ctx);
    let resolvers_for_task = resolvers.clone();

    let opts = match parse_request_opts(args, ctx) {
        Ok(o) => o,
        Err(msg) => {
            let e = JsObject::with_object_proto(ctx.intrinsics());
            let _ = e.create_data_property(
                js_string!("message"),
                JsValue::from(js_string!(msg.as_str())),
                ctx,
            );
            let _ = resolvers
                .reject
                .call(&JsValue::undefined(), &[e.into()], ctx);
            return Ok(make_task(ctx, &promise, &resolvers_for_task, None, None).into());
        }
    };

    // Cancellation state, shared between the Task handle (created now, before
    // the stream exists) and the StreamHandle built by the completion below.
    // `extra_cancel` is the wire abort: flip the flag + drop the pipe.
    let cancelled: SharedFlag = Rc::new(Cell::new(false));
    let stream_slot: SharedStream = Rc::new(RefCell::new(None));
    let extra_cancel = {
        let cancelled = cancelled.clone();
        let stream_slot = stream_slot.clone();
        Box::new(move || {
            cancelled.set(true);
            *stream_slot.borrow_mut() = None;
        })
    };

    let completion_handle_for_complete = completion_handle.clone();
    let js_ctx_for_spawn = worker_sched.clone();
    let handle = js_ctx.spawn_local(move |_aw| async move {
        match http.request_stream(opts).await {
            Ok(resp) => {
                let status = resp.status;
                let status_text = resp.status_text;
                let headers = resp.headers;
                let body_stream = resp.body;
                let js_ctx_clone = js_ctx_for_spawn.clone();
                let completion_handle_clone = completion_handle_for_complete.clone();
                let cancelled_check = cancelled.clone();
                let stream_slot_for_build = stream_slot.clone();

                completion_handle_for_complete.push(Box::new(move |ctx| {
                    // Cancelled mid-flight (the task promise already
                    // rejected with a CancelError) — drop the pipe unseen.
                    if cancelled_check.get() {
                        return Ok(());
                    }
                    *stream_slot_for_build.borrow_mut() = Some(body_stream);
                    build_stream_response(
                        status,
                        &status_text,
                        &headers,
                        stream_slot_for_build,
                        cancelled_check,
                        js_ctx_clone,
                        completion_handle_clone,
                        &resolvers,
                        ctx,
                    )?;
                    Ok(())
                }));
            }
            Err(e) => {
                let cancelled_check = cancelled.clone();
                completion_handle_for_complete.push(Box::new(move |ctx| {
                    if cancelled_check.get() {
                        return Ok(());
                    }
                    let err_obj = JsObject::with_object_proto(ctx.intrinsics());
                    let _ = err_obj.create_data_property(
                        js_string!("message"),
                        JsValue::from(js_string!(e.as_str())),
                        ctx,
                    );
                    let _ = resolvers
                        .reject
                        .call(&JsValue::undefined(), &[err_obj.into()], ctx)?;
                    Ok(())
                }));
            }
        }
    });

    Ok(make_task(
        ctx,
        &promise,
        &resolvers_for_task,
        Some(handle),
        Some(extra_cancel),
    )
    .into())
}

/// Build the resolved `{ ok, status, statusText, headers, body }` object where
/// `body` is a `JsObject` carrying the stream state + `[Symbol.asyncIterator]`
/// + `next()` methods.
#[allow(clippy::too_many_arguments)]
fn build_stream_response(
    status: u16,
    status_text: &str,
    headers: &[(String, String)],
    stream: SharedStream,
    cancelled: SharedFlag,
    js_ctx: TurInstanceContext,
    completion_handle: CompletionHandle,
    resolvers: &boa_engine::builtins::promise::ResolvingFunctions,
    ctx: &mut Context,
) -> JsResult<()> {
    let o = JsObject::with_object_proto(ctx.intrinsics());
    let _ = o.create_data_property(js_string!("ok"), JsValue::from(true), ctx);
    let _ = o.create_data_property(js_string!("status"), JsValue::from(status as f64), ctx);
    let _ = o.create_data_property(
        js_string!("statusText"),
        JsValue::from(js_string!(status_text)),
        ctx,
    );

    let hmap = JsObject::with_object_proto(ctx.intrinsics());
    for (k, v) in headers {
        let _ = hmap.create_data_property(
            js_string!(k.as_str()),
            JsValue::from(js_string!(v.as_str())),
            ctx,
        );
    }
    let _ = o.create_data_property(js_string!("headers"), JsValue::from(hmap), ctx);

    // Body object: JsData = StreamHandle, with next() +
    // [Symbol.asyncIterator]. Stream abort lives on the Task handle
    // (`requestStream(...).cancel()` flips `cancelled` + drops the pipe
    // slot), not on the body itself — the body is just an async iterator.
    let handle = StreamHandle {
        stream,
        js_ctx,
        completion_handle,
        cancelled,
        next_in_flight: Rc::new(Cell::new(false)),
    };
    let proto = ctx.intrinsics().constructors().object().prototype();
    let body = JsObject::from_proto_and_data(proto, handle);

    let next_fn = NativeFunction::from_fn_ptr(tur_stream_next);
    let next_obj = FunctionObjectBuilder::new(ctx.realm(), next_fn)
        .length(0)
        .name(js_string!("next"))
        .build();
    let _ = body.create_data_property(js_string!("next"), JsValue::from(next_obj), ctx);

    let iter_fn = NativeFunction::from_fn_ptr(tur_stream_async_iterator);
    let iter_obj = FunctionObjectBuilder::new(ctx.realm(), iter_fn)
        .length(0)
        .name(js_string!("[Symbol.asyncIterator]"))
        .build();
    let _ = body.create_data_property(
        PropertyKey::Symbol(JsSymbol::async_iterator()),
        JsValue::from(iter_obj),
        ctx,
    );

    let _ = o.create_data_property(js_string!("body"), JsValue::from(body), ctx);

    resolvers
        .resolve
        .call(&JsValue::undefined(), &[o.into()], ctx)?;
    Ok(())
}

/// `[Symbol.asyncIterator]()` — returns `this` (the body object is its own
/// async iterator).
fn tur_stream_async_iterator(
    this: &JsValue,
    _args: &[JsValue],
    _ctx: &mut Context,
) -> JsResult<JsValue> {
    Ok(this.clone())
}

/// `next(): Promise<{done, value}>` — reads one chunk from the stream. Resolves
/// with `{done:false, value:Uint8Array}` for each chunk, or `{done:true}` when
/// the stream ends or the owning Task was cancelled. Rejects on I/O error, and
/// on a call made while a previous one is still pending (the pull protocol is
/// serial — that's what carries backpressure).
fn tur_stream_next(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("stream.next called on non-object"))
    })?;
    let handle = obj.downcast_ref::<StreamHandle>().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("stream.next called on non-stream object"))
    })?;

    let (promise, resolvers) = JsPromise::new_pending(ctx);

    // One outstanding pull at a time — checked BEFORE the exhausted check,
    // because an in-flight poll also leaves the slot temporarily empty (the
    // stream is taken out while being polled). A second concurrent call used
    // to resolve `{done: true}` spuriously — misreporting a stall as
    // end-of-stream.
    if handle.next_in_flight.replace(true) {
        reject_stream_error(
            &resolvers,
            "stream.next() called while a previous call is still pending",
            ctx,
        )?;
        return Ok(promise.into());
    }

    // Cancelled or already exhausted — done immediately.
    if handle.cancelled.get() || handle.stream.borrow().is_none() {
        handle.next_in_flight.set(false);
        resolve_stream_done(&resolvers, ctx)?;
        return Ok(promise.into());
    }

    // Take the stream out so we can poll it inside the spawned future.
    let stream_opt = handle.stream.borrow_mut().take();
    let stream_rc = handle.stream.clone();
    let js_ctx = handle.js_ctx.clone();
    let completion_handle = handle.completion_handle.clone();
    let cancelled = handle.cancelled.clone();
    let next_in_flight = handle.next_in_flight.clone();

    let _ = js_ctx.spawn_local(|_aw| async move {
        let mut s = stream_opt;
        let polled = match s.as_mut() {
            Some(stream) => stream.next().await,
            None => None,
        };

        // The stream slot is restored inside the completion (not here) so a
        // `cancel()` landing while this poll is in flight wins: the pipe is
        // dropped, never revived.
        match polled {
            Some(Ok(chunk)) => {
                completion_handle.push(Box::new(move |ctx| {
                    next_in_flight.set(false);
                    if cancelled.get() {
                        return resolve_stream_done(&resolvers, ctx);
                    }
                    *stream_rc.borrow_mut() = s;
                    let result = JsObject::with_object_proto(ctx.intrinsics());
                    let _ =
                        result.create_data_property(js_string!("done"), JsValue::from(false), ctx);
                    let u8a = JsUint8Array::from_iter(chunk, ctx)?;
                    let _ =
                        result.create_data_property(js_string!("value"), JsValue::from(u8a), ctx);
                    resolvers
                        .resolve
                        .call(&JsValue::undefined(), &[result.into()], ctx)?;
                    Ok(())
                }));
            }
            Some(Err(e)) => {
                completion_handle.push(Box::new(move |ctx| {
                    next_in_flight.set(false);
                    if cancelled.get() {
                        return resolve_stream_done(&resolvers, ctx);
                    }
                    *stream_rc.borrow_mut() = s;
                    reject_stream_error(&resolvers, e.as_str(), ctx)
                }));
            }
            None => {
                // Stream ended — leave the slot empty (the stream is dropped).
                completion_handle.push(Box::new(move |ctx| {
                    next_in_flight.set(false);
                    resolve_stream_done(&resolvers, ctx)
                }));
            }
        }
    });

    Ok(promise.into())
}
