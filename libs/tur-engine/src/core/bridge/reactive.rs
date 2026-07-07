use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsArgs, JsResult, JsValue};

use crate::core::bridge::helpers::{extract_ctx, FnEntry, Ptr};
use crate::core::bridge::BoaOpaque;
use crate::core::reactive::{
    AnyReadable, Derived, FromBoaJsValue, IntoBoaJsValue, Mutation, Readable, Source,
};

/// Bridge function table entries for the reactive primitives domain.
pub fn fns() -> Vec<FnEntry> {
    vec![
        ("source", 2, tur_source as Ptr),
        ("derive", 2, tur_derive as Ptr),
        ("mutate", 2, tur_mutate as Ptr),
        ("get", 2, tur_get as Ptr),
        ("set", 3, tur_set as Ptr),
        ("view", 1, tur_view as Ptr),
    ]
}

fn require_callable(args: &[JsValue], idx: usize) -> JsResult<JsFunction> {
    let v = args.get_or_undefined(idx);
    let obj = v.as_object().ok_or_else(|| {
        boa_engine::JsError::from(
            boa_engine::JsNativeError::typ().with_message("expected a function"),
        )
    })?;
    JsFunction::from_object(obj.clone()).ok_or_else(|| {
        boa_engine::JsError::from(
            boa_engine::JsNativeError::typ().with_message("expected a function"),
        )
    })
}

fn tur_source(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let value = args.get_or_undefined(1).clone();
    let source: Source<JsValue> = js_ctx.store.bridge().source(value);
    Ok(source.into_js(context))
}

fn tur_derive(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let closure = require_callable(args, 1)?;
    let derived: Derived<JsValue> = js_ctx.store.bridge().derive(closure);
    Ok(derived.into_js(context))
}

fn tur_mutate(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let closure = require_callable(args, 1)?;
    let mutation = js_ctx.store.bridge().mutate(closure);
    Ok(mutation.into_js(context))
}

fn tur_get(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let readable = AnyReadable::from_js(args.get_or_undefined(1)).ok_or_else(|| {
        boa_engine::JsError::from(
            boa_engine::JsNativeError::typ()
                .with_message("expected a source or derived atom handle"),
        )
    })?;
    Ok(js_ctx.store.bridge().read(readable, context))
}

fn tur_set(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let bridge = js_ctx.store.bridge();
    let v = args.get_or_undefined(1);
    if let Some(mutation) = Mutation::from_js(v) {
        let ctx_obj = bridge.ctx_object(context)?;
        let mut invoke_args: Vec<JsValue> = Vec::with_capacity(args.len() + 1);
        invoke_args.push(ctx_obj.into());
        if let Some(extra) = args.get(2..) {
            invoke_args.extend_from_slice(extra);
        }
        return bridge.invoke_mutation(mutation, &invoke_args, context);
    }
    if let Some(readable) = AnyReadable::from_js(v) {
        return match readable {
            Readable::Source(source) => {
                let value = args.get_or_undefined(2).clone();
                bridge.set_source(source, value);
                Ok(JsValue::undefined())
            }
            Readable::Derived(_) => Err(boa_engine::JsError::from(
                boa_engine::JsNativeError::typ().with_message("cannot set a derived atom"),
            )),
        };
    }
    Err(boa_engine::JsError::from(
        boa_engine::JsNativeError::typ().with_message("expected an atom handle"),
    ))
}

/// `view(factory)` — wrap a JS thunk `() => Element` as a `JsView`
/// (a View) and return it as a `ViewHandle`. The thunk is invoked
/// lazily when the view is built (transparent pass-through).
fn tur_view(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let f = require_callable(args, 1)?;
    let view: Rc<dyn crate::core::view::View> =
        Rc::new(crate::core::view::JsView(f));
    let handle = crate::core::view::ViewHandle::new(view);
    let opaque = BoaOpaque::new(handle, context);
    Ok(opaque.object().clone().into())
}
