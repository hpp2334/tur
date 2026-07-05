//! JS bridge for `lifecycleView(factory)`.

use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};

use crate::core::bridge::helpers::{extract_ctx, wrap_view, FnEntry, Ptr};
use crate::core::view::View;

pub fn fns() -> Vec<FnEntry> {
    vec![("lifecycleView", 1, tur_lifecycle_view as Ptr)]
}

/// `lifecycleView(factory)` — wraps a JS factory
/// `() => { element, onMounted$?, beforeDestroy$? }` as a `LifecycleView`.
/// The factory is invoked lazily at build time.
fn tur_lifecycle_view(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let v = args.get_or_undefined(1);
    let obj = v
        .as_object()
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message("expected a function")))?;
    let factory = JsFunction::from_object(obj.clone())
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message("expected a function")))?;
    let view: Rc<dyn View> = Rc::new(super::LifecycleView { factory });
    Ok(wrap_view(view, context))
}
