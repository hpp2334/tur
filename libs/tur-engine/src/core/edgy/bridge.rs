use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsArgs, JsResult, JsValue};

use crate::core::edgy::reactive::{Derived, Source, make_store_js_object};
use crate::core::js_runtime::helpers::{FnEntry, Ptr, extract_js_ctx};
use crate::core::js_runtime::js_value::IntoJs;

/// Bridge function table entries for the reactive primitives domain.
///
/// `source` / `derive` / `mutate` mint **declarations** — pure handles that
/// hold no state; a store materializes them on first touch. Reading/writing
/// happens through a store (`createStore()` object's `get` / `set`) or the
/// `{get, set}` ctx handed to derive/mutate closures — there are no
/// module-level `get` / `set` functions and no way to grab "the current
/// store" (embedded code threads ctx through from its closures).
pub fn fns() -> Vec<FnEntry> {
    vec![
        ("source", 2, tur_source as Ptr),
        ("derive", 2, tur_derive as Ptr),
        ("mutate", 2, tur_mutate as Ptr),
        ("createStore", 1, tur_create_store as Ptr),
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

fn tur_source(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let value = args.get_or_undefined(1).clone();
    let source: Source<JsValue> = js_ctx.store.bridge().decl_source(value);
    Ok(source.into_js(context))
}

fn tur_derive(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let closure = require_callable(args, 1)?;
    let derived: Derived<JsValue> = js_ctx.store.bridge().decl_derive(closure);
    Ok(derived.into_js(context))
}

fn tur_mutate(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let closure = require_callable(args, 1)?;
    let mutation = js_ctx.store.bridge().decl_mutate(closure);
    Ok(mutation.into_js(context))
}

/// `createStore()` — mint a fresh store over this instance's shared reactive
/// machinery. The store is a real KV: a declaration materialized here is
/// independent of the same declaration materialized in another store.
fn tur_create_store(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let store = js_ctx.store.spawn();
    let obj = make_store_js_object(context, store);
    Ok(obj.into())
}

/// `view(factory)` — wrap a JS thunk `() => Element` as a `JsView`
/// (a View) and return it as a `ViewHandle`. The thunk is invoked
/// lazily when the view is built (transparent pass-through).
fn tur_view(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let f = require_callable(args, 1)?;
    let view = Rc::new(crate::core::view::JsView(f));
    Ok(crate::core::js_runtime::helpers::wrap_view(view, context))
}
