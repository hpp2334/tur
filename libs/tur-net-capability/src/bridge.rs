//! `tur:net` HTTP bridge: `request(opts) -> Promise<ResponseResult>`.
//!
//! Mirrors the clipboard bridge pattern in tur-clipboard: a **ctx-bound fn
//! pointer** (no captures) that reads its `Rc<dyn Http>` + scheduler
//! primitives from `TurJsContext`. The fn creates a pending `JsPromise`,
//! spawns a future via [`WorkerScheduler::spawn_local`] that calls
//! `Http::request(opts).await`, and pushes a completion closure that
//! builds the JS response object and resolves/rejects the promise under
//! `&mut Context`.
//!
//! This file contains **no `unsafe`** — uses `NativeFunction::from_fn_ptr`
//! via the engine's `bound_native` helper instead of the previous
//! `unsafe NativeFunction::from_closure`. Captures are eliminated because
//! the needed state lives in the capability registry (populated by
//! [`crate::TurNetPlugin`] during `register`).

use std::cell::RefCell;
use std::pin::Pin;
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

use tur_engine::core::async_::CompletionHandle;
use tur_engine::core::js_runtime::helpers::{FnEntry, Ptr, extract_ctx};
use tur_engine::core::scheduler::WorkerScheduler;

/// Shorthand for the boxed byte-chunk stream used by the streaming bridge.
type ByteChunkStream = LocalBoxStream<'static, Result<Vec<u8>, String>>;

/// Shared stream state — `RefCell<Option<…>>` so `next()` can take the stream
/// out, poll one chunk, and put it back.
type SharedStream = Rc<RefCell<Option<ByteChunkStream>>>;

use crate::{Http, HttpBody, HttpOutcome, RequestOpts, ResponseType};
/// Bridge function tables entries for `tur:net`.
///
/// Returns `("request", 1, tur_net_request as Ptr)` — a ctx-bound fn pointer
/// that reads its `Http` + scheduler from `TurJsContext`.
pub fn fns() -> Vec<FnEntry> {
    vec![
        ("request", 1, tur_net_request as Ptr),
        ("requestStream", 1, tur_net_request_stream as Ptr),
    ]
}

/// `request(opts): Promise<ResponseResult>` — performs an HTTP request via
/// the injected `Http` backend. Rejects with `{ message }` on network error
/// or when no backend is registered.
fn tur_net_request(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let http = js_ctx
        .capability()
        .of::<Http>()
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message("no http capability")))?
        .backend()
        .clone();
    let worker_sched = js_ctx.worker_sched().clone();
    let completion_handle = js_ctx.completion_handle();

    let (promise, resolvers) = JsPromise::new_pending(ctx);

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
            return Ok(promise.into());
        }
    };

    let fut: Pin<Box<dyn std::future::Future<Output = ()> + 'static>> = Box::pin(async move {
        let outcome = http.request(opts).await;
        completion_handle.push(Box::new(move |ctx| {
            resolve_outcome(&outcome, &resolvers, ctx)?;
            Ok(())
        }));
    });
    worker_sched.spawn_local(fut);
    Ok(promise.into())
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
    })
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
    worker_sched: Rc<dyn WorkerScheduler>,
    completion_handle: CompletionHandle,
}

/// `requestStream(opts): Promise<StreamResponse>` — performs a streaming HTTP
/// request. The resolved value has `{ ok, status, statusText, headers, body }`
/// where `body` is an async iterable yielding `Uint8Array` chunks.
fn tur_net_request_stream(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let http = js_ctx
        .capability()
        .of::<Http>()
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message("no http capability")))?
        .backend()
        .clone();
    let worker_sched = js_ctx.worker_sched().clone();
    let completion_handle = js_ctx.completion_handle();

    let (promise, resolvers) = JsPromise::new_pending(ctx);

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
            return Ok(promise.into());
        }
    };

    let completion_handle_for_complete = completion_handle.clone();
    let worker_sched_for_spawn = worker_sched.clone();
    let fut: Pin<Box<dyn std::future::Future<Output = ()> + 'static>> = Box::pin(async move {
        match http.request_stream(opts).await {
            Ok(resp) => {
                let status = resp.status;
                let status_text = resp.status_text;
                let headers = resp.headers;
                let stream_rc = Rc::new(RefCell::new(Some(resp.body)));
                let worker_sched_clone = worker_sched_for_spawn.clone();
                let completion_handle_clone = completion_handle_for_complete.clone();

                completion_handle_for_complete.push(Box::new(move |ctx| {
                    build_stream_response(
                        status,
                        &status_text,
                        &headers,
                        stream_rc,
                        worker_sched_clone,
                        completion_handle_clone,
                        &resolvers,
                        ctx,
                    )?;
                    Ok(())
                }));
            }
            Err(e) => {
                completion_handle_for_complete.push(Box::new(move |ctx| {
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
    worker_sched.spawn_local(fut);

    Ok(promise.into())
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
    worker_sched: Rc<dyn WorkerScheduler>,
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

    // Body object: JsData = StreamHandle, with next() + [Symbol.asyncIterator]
    let handle = StreamHandle {
        stream,
        worker_sched,
        completion_handle,
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
/// the stream ends. Rejects on I/O error.
fn tur_stream_next(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("stream.next called on non-object"))
    })?;
    let handle = obj.downcast_ref::<StreamHandle>().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("stream.next called on non-stream object"))
    })?;

    let (promise, resolvers) = JsPromise::new_pending(ctx);

    // If the stream is already exhausted, resolve immediately with {done:true}.
    if handle.stream.borrow().is_none() {
        let result = JsObject::with_object_proto(ctx.intrinsics());
        let _ = result.create_data_property(js_string!("done"), JsValue::from(true), ctx);
        let _ = resolvers
            .resolve
            .call(&JsValue::undefined(), &[result.into()], ctx);
        return Ok(promise.into());
    }

    // Take the stream out so we can poll it inside the spawned future.
    let stream_opt = handle.stream.borrow_mut().take();
    let stream_rc = handle.stream.clone();
    let sched = handle.worker_sched.clone();
    let completion_handle = handle.completion_handle.clone();

    let fut: Pin<Box<dyn std::future::Future<Output = ()> + 'static>> = Box::pin(async move {
        let mut s = stream_opt;
        let polled = match s.as_mut() {
            Some(stream) => stream.next().await,
            None => None,
        };

        match polled {
            Some(Ok(chunk)) => {
                *stream_rc.borrow_mut() = s;
                completion_handle.push(Box::new(move |ctx| {
                    let result = JsObject::with_object_proto(ctx.intrinsics());
                    let _ =
                        result.create_data_property(js_string!("done"), JsValue::from(false), ctx);
                    let u8a = JsUint8Array::from_iter(chunk, ctx)?;
                    let _ =
                        result.create_data_property(js_string!("value"), JsValue::from(u8a), ctx);
                    let _ = resolvers
                        .resolve
                        .call(&JsValue::undefined(), &[result.into()], ctx)?;
                    Ok(())
                }));
            }
            Some(Err(e)) => {
                *stream_rc.borrow_mut() = s;
                completion_handle.push(Box::new(move |ctx| {
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
            None => {
                // Stream ended — leave stream as None
                completion_handle.push(Box::new(move |ctx| {
                    let result = JsObject::with_object_proto(ctx.intrinsics());
                    let _ =
                        result.create_data_property(js_string!("done"), JsValue::from(true), ctx);
                    let _ = resolvers
                        .resolve
                        .call(&JsValue::undefined(), &[result.into()], ctx)?;
                    Ok(())
                }));
            }
        }
    });
    sched.spawn_local(fut);

    Ok(promise.into())
}
