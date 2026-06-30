use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsArgs, JsResult, JsValue};

use crate::core::bridge::{BoaOpaque, TurJsContext};
use crate::core::reactive::{
    extract_handle, AtomHandle, AtomKind, Mutation, Source,
};

fn extract_ctx(args: &[JsValue]) -> JsResult<TurJsContext> {
    let obj = args.get_or_undefined(0).as_object().ok_or_else(|| {
        boa_engine::JsError::from(
            boa_engine::JsNativeError::typ()
                .with_message("expected TurJsContext as first argument"),
        )
    })?;
    let ctx_ref = BoaOpaque::<TurJsContext>::wrap(&obj).ok_or_else(|| {
        boa_engine::JsError::from(
            boa_engine::JsNativeError::typ()
                .with_message("expected TurJsContext as first argument"),
        )
    })?;
    Ok(ctx_ref.clone())
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

pub(crate) fn tur_source(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let value = args.get_or_undefined(1).clone();
    let source = js_ctx.store.bridge().source::<JsValue>(value);
    let opaque = BoaOpaque::new(AtomHandle::new(source.id(), AtomKind::Source), context);
    Ok(opaque.object().clone().into())
}

pub(crate) fn tur_derive(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let closure = require_callable(args, 1)?;
    let derived = js_ctx.store.bridge().derive::<JsValue>(closure);
    let opaque = BoaOpaque::new(AtomHandle::new(derived.id(), AtomKind::Derived), context);
    Ok(opaque.object().clone().into())
}

pub(crate) fn tur_mutate(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let closure = require_callable(args, 1)?;
    let mutation = js_ctx.store.bridge().mutate(closure);
    let opaque = BoaOpaque::new(AtomHandle::new(mutation.0, AtomKind::Mutation), context);
    Ok(opaque.object().clone().into())
}

pub(crate) fn tur_get(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let readable = crate::core::reactive::require_readable::<JsValue>(args, 1)?;
    Ok(js_ctx.store.bridge().read(readable, context))
}

pub(crate) fn tur_set(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let bridge = js_ctx.store.bridge();
    let handle = extract_handle(args.get_or_undefined(1)).ok_or_else(|| {
        boa_engine::JsError::from(
            boa_engine::JsNativeError::typ().with_message("expected an atom handle"),
        )
    })?;
    match handle.kind {
        AtomKind::Mutation => {
            let ctx_obj = bridge.ctx_object(context)?;
            let mut invoke_args: Vec<JsValue> = Vec::with_capacity(args.len() + 1);
            invoke_args.push(ctx_obj.into());
            if let Some(extra) = args.get(2..) {
                invoke_args.extend_from_slice(extra);
            }
            bridge.invoke_mutation(Mutation(handle.id), &invoke_args, context)
        }
        AtomKind::Source => {
            let value = args.get_or_undefined(2).clone();
            bridge.set_source(Source::<JsValue>::from_id(handle.id), value);
            Ok(JsValue::undefined())
        }
        AtomKind::Derived => Err(boa_engine::JsError::from(
            boa_engine::JsNativeError::typ().with_message("cannot set a derived atom"),
        )),
    }
}

/// `view(factory)` — wrap a JS thunk `() => EdgyElement` as a `JsView`
/// (a View) and return it as a `ViewHandle`. The thunk is invoked
/// lazily when the view is built (transparent pass-through).
pub(crate) fn tur_view(
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
