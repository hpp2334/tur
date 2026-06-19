use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsArgs, JsResult, JsValue};

use crate::core::bridge::{BoaOpaque, TurJsContext};
use crate::core::reactive::{build_store_context_object, AtomHandle, Store};

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
    let id = js_ctx.store.borrow().source(value);
    let opaque = BoaOpaque::new(AtomHandle::new(id), context);
    Ok(opaque.object().clone().into())
}

pub(crate) fn tur_derive(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let closure = require_callable(args, 1)?;
    let store_ctx_obj = ensure_store_ctx_object(&js_ctx, context)?;
    let id = js_ctx
        .store
        .borrow()
        .derive(closure, context, &store_ctx_obj);
    let opaque = BoaOpaque::new(AtomHandle::new(id), context);
    Ok(opaque.object().clone().into())
}

pub(crate) fn tur_mutate(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let closure = require_callable(args, 1)?;
    let id = js_ctx.store.borrow().mutate(closure);
    let opaque = BoaOpaque::new(AtomHandle::new(id), context);
    Ok(opaque.object().clone().into())
}

pub(crate) fn tur_get(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let id = crate::core::reactive::require_atom(args, 1)?;
    Ok(js_ctx.store.borrow().get_tracked(id, context))
}

pub(crate) fn tur_set(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let id = crate::core::reactive::require_atom(args, 1)?;
    let store = js_ctx.store.borrow();
    match store.kind_of(id) {
        Some(crate::core::reactive::AtomKind::Mutation) => {
            let extra: Vec<JsValue> = args.get(2..).map(|s| s.to_vec()).unwrap_or_default();
            store.invoke_mutation(id, &extra, context)
        }
        _ => {
            let value = args.get_or_undefined(2).clone();
            store.set_source(id, value);
            Ok(JsValue::undefined())
        }
    }
}

/// `component(factory)` — wrap a JS thunk `() => EdgyElement` as a `JsComponent`
/// (a Component) and return it as a `ComponentHandle`. The thunk is invoked
/// lazily when the component is built (transparent pass-through).
pub(crate) fn tur_component(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let f = require_callable(args, 1)?;
    let component: Rc<dyn crate::core::widget::Component> =
        Rc::new(crate::core::widget::JsComponent(f));
    let handle = crate::core::widget::ComponentHandle::new(component);
    let opaque = BoaOpaque::new(handle, context);
    Ok(opaque.object().clone().into())
}

/// Build (or fetch the cached) per-store `{ get, set }` JS context object.
/// Stored as a boa `JsData` on a sentinel opaque so we don't rebuild per
/// invocation.
pub fn ensure_store_ctx_object(
    js_ctx: &TurJsContext,
    context: &mut Context,
) -> JsResult<JsValue> {
    // Rebuild on every call — the closure capture pattern is cheap enough.
    // (Could be cached, but doing so requires GC rooting; deferred.)
    let obj = build_store_context_object(context, js_ctx.store.clone())?;
    Ok(obj.into())
}

/// Convenience used internally to construct the store ctx object given a raw
/// `Rc<RefCell<Store>>`. Used by the flush loop.
pub fn build_ctx_object_for(
    store: Rc<RefCell<Store>>,
    context: &mut Context,
) -> JsResult<JsValue> {
    let obj = build_store_context_object(context, store)?;
    Ok(obj.into())
}
