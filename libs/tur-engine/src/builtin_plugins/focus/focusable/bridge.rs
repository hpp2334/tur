//! JS bridge for the `Focusable` element + `requestFocus`.

use std::rc::Rc;

use boa_engine::{Context, JsArgs, JsNativeError, JsResult, JsValue};

use crate::core::js_runtime::BoaOpaque;
use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, TurNodeHandle, extract_ctx, require_props_object, wrap_view,
};

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("Focusable", 2, tur_focusable as Ptr),
        ("requestFocus", 2, tur_request_focus as Ptr),
    ]
}

fn tur_focusable(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::FocusableView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_request_focus(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let obj = args
        .get_or_undefined(1)
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("expected TurNodeHandle"))?;
    let handle = BoaOpaque::<TurNodeHandle>::wrap(&obj)
        .ok_or_else(|| JsNativeError::typ().with_message("expected TurNodeHandle"))?;
    let mut focus = js_ctx.focus_manager.borrow_mut();
    focus.set_focus(handle.id);
    Ok(JsValue::undefined())
}
