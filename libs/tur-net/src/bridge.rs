//! `builtin:tur/net` HTTP bridge: `request(opts) -> Promise<ResponseResult>`.
//!
//! Mirrors the clipboard bridge pattern in tur-std: a closure that creates a
//! pending `JsPromise`, spawns a future via the engine's `AsyncExecutor` that
//! calls `Http::request(opts).await`, and pushes a completion closure that
//! builds the JS response object and resolves/rejects the promise under
//! `&mut Context`.
//!
//! Like clipboard, this is a free-form closure (not a ctx-bound fn pointer)
//! because it captures `Rc<dyn Http>` and `Rc<AsyncExecutor>`, neither of
//! which can live on `TurJsContext`.

use std::rc::Rc;

use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::{JsArrayBuffer, JsPromise};
use boa_engine::object::JsObject;
use boa_engine::property::PropertyKey;
use boa_engine::{js_string, JsArgs, JsResult, JsValue};

use tur_engine::core::async_::AsyncExecutor;

use crate::{Http, HttpBody, HttpOutcome, RequestOpts, ResponseType};

/// Build the `request` bridge closure for `builtin:tur/net`.
///
/// Returns `(name, length, NativeFunction)` entries. The caller (typically
/// [`crate::TurNetPlugin`]) re-orders to `(name, fn, length)` for
/// [`tur_engine::core::plugin::PluginContext::register_host_module`].
pub fn closures(
    http: Rc<dyn Http>,
    executor: Rc<AsyncExecutor>,
) -> Vec<(&'static str, usize, NativeFunction)> {
    vec![("request", 1, build_request(http, executor))]
}

fn build_request(http: Rc<dyn Http>, executor: Rc<AsyncExecutor>) -> NativeFunction {
    // SAFETY: captures are pure Rust state (`Rc<dyn Http>`, `Rc<AsyncExecutor>`)
    // — no boa GC-traceable types. Sound to use `from_closure`.
    unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let (promise, resolvers) = JsPromise::new_pending(ctx);

            // Parse opts from the JS `{ url, method?, ... }` object.
            let opts = match parse_request_opts(args, ctx) {
                Ok(o) => o,
                Err(msg) => {
                    let e = JsObject::with_object_proto(ctx.intrinsics());
                    let _ = e.create_data_property(
                        js_string!("message"),
                        JsValue::from(js_string!(msg.as_str())),
                        ctx,
                    );
                    let _ =
                        resolvers
                            .reject
                            .call(&JsValue::undefined(), &[e.into()], ctx);
                    return Ok(promise.into());
                }
            };

            let http = http.clone();
            let executor = executor.clone();
            let executor_for_complete = executor.clone();
            executor.spawn_detached(async move {
                let outcome = http.request(opts).await;
                executor_for_complete.complete(Box::new(move |ctx| {
                    resolve_outcome(&outcome, &resolvers, ctx)?;
                    Ok(())
                }));
            });
            Ok(promise.into())
        })
    }
}

fn parse_request_opts(
    args: &[JsValue],
    ctx: &mut boa_engine::Context,
) -> Result<RequestOpts, String> {
    let opts = args.get_or_undefined(0);
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

fn js_opt_str(
    obj: &JsObject,
    key: &str,
    ctx: &mut boa_engine::Context,
) -> Option<String> {
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
            let _ = o.create_data_property(
                js_string!("status"),
                JsValue::from(*status as f64),
                ctx,
            );
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
                        let _ = o.create_data_property(
                            js_string!("bodyBytes"),
                            JsValue::from(ab),
                            ctx,
                        );
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
