//! Shared bridge plumbing used by every per-domain bridge file
//! (`elements/<name>/bridge.rs`, `core/bridge/reactive.rs`, etc.).

use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::js_runtime::{BoaOpaque, TurJsContext};
use crate::core::element::ElementNodeId;
use crate::core::view::{View, ViewHandle};

/// Native function pointer type used by the bridge table.
pub type Ptr = boa_engine::native_function::NativeFunctionPointer;

/// A bridge function table entry: `(js_name, length, native_fn_pointer)`.
pub type FnEntry = (&'static str, usize, Ptr);

/// A constant module export entry: `(js_name, value)`.
pub type ConstEntry = (&'static str, JsValue);

/// A JS-opaque handle wrapping an [`ElementNodeId`], used by controllers
/// (`ScrollController`, `TextEditingController`, …) and `requestFocus` to
/// reference an element without exposing the id as a plain number.
#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurNodeHandle {
    pub id: ElementNodeId,
}

/// Decode the bound bridge ctx (prepended by `bound_native`) from `args[0]`.
pub fn extract_ctx(args: &[JsValue]) -> JsResult<TurJsContext> {
    let obj = args.get_or_undefined(0).as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected TurJsContext as first argument"))
    })?;
    let ctx_ref = BoaOpaque::<TurJsContext>::wrap(&obj).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected TurJsContext as first argument"))
    })?;
    Ok(ctx_ref.clone())
}

/// Coerce `args[idx]` to a `JsObject`, treating `null`/`undefined` as `{}`.
pub fn require_props_object(
    args: &[JsValue],
    idx: usize,
    context: &mut Context,
) -> JsResult<JsObject> {
    let v = args.get_or_undefined(idx);
    if v.is_undefined() || v.is_null() {
        let proto = context.intrinsics().constructors().object().prototype();
        return Ok(JsObject::from_proto_and_data(proto, ()));
    }
    let obj = v
        .as_object()
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message("expected props object")))?;
    Ok(obj.clone())
}

/// Wrap a built `View` spec as a JS-opaque `ViewHandle` value.
pub fn wrap_view(spec: Rc<dyn View>, context: &mut Context) -> JsValue {
    let opaque = BoaOpaque::new(ViewHandle::new(spec), context);
    opaque.object().clone().into()
}
