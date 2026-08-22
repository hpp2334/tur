use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsArgs, JsResult, JsValue, js_string};

use crate::core::edgy::reactive::{AnyReadable, Derived, Mutation, Source};
use crate::core::js_runtime::helpers::{FnEntry, Ptr, extract_js_ctx};
use crate::core::js_runtime::js_value::{FromJs, IntoJs};

/// Bridge function table entries for the reactive primitives domain.
///
/// `source` / `derive` / `mutate` mint **declarations** — pure handles that
/// hold no state; a store materializes them on first touch. Reading/writing
/// happens through the instance store (the `{get, set}` object the engine
/// hands to the module's `start({ store })`) or the `{get, set}` ctx handed
/// to derive/mutate closures — there are no module-level `get` / `set`
/// functions, no way to create further stores (one instance, one store), and
/// no way to grab "the current store" outside a closure (embedded code
/// threads ctx through from its closures).
///
/// `watch(readable, cb)` registers a non-element subscriber over an atom and
/// returns `{ start$, stop$ }` control mutations (see `edgy::watch`).
pub fn fns() -> Vec<FnEntry> {
    vec![
        ("source", 2, tur_source as Ptr),
        ("derive", 2, tur_derive as Ptr),
        ("mutate", 2, tur_mutate as Ptr),
        ("watch", 3, tur_watch as Ptr),
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

/// `watch(readable, cb)` — register a watcher over a source or derived atom
/// and return `{ start$, stop$ }`. `cb` is a **mutation handle** (create one
/// with `mutate((ctx) => …)` — the same convention as `onTick` /
/// `onUpdate$` / `onClick`); while started, the flush loop invokes it
/// whenever the watched atom is dirtied (same rail as every other mutation
/// — mounted-store ctx, same-frame delivery). Change-only: starting does not
/// fire it.
fn tur_watch(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let watched = AnyReadable::from_js(args.get_or_undefined(1))?;
    let callback = Mutation::from_js(args.get_or_undefined(2)).map_err(|_| {
        boa_engine::JsError::from(boa_engine::JsNativeError::typ().with_message(
            "watch(atom, cb) expects a mutation handle — create one with mutate((ctx) => …)",
        ))
    })?;
    let bridge = js_ctx.store.bridge();
    let (start, stop) = bridge.register_watch(watched, callback);

    let proto = context.intrinsics().constructors().object().prototype();
    let obj = JsObject::from_proto_and_data(proto, ());
    obj.create_data_property_or_throw(js_string!("start$"), start.into_js(context), context)?;
    obj.create_data_property_or_throw(js_string!("stop$"), stop.into_js(context), context)?;
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
