//! JS bridge for the `ReadableSubscribe` element.

use std::rc::Rc;

use boa_engine::{Context, JsResult, JsValue};

use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_ctx, require_props_object, wrap_view,
};

pub fn fns() -> Vec<FnEntry> {
    vec![("ReadableSubscribe", 2, tur_readable_subscribe as Ptr)]
}

fn tur_readable_subscribe(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::ReadableSubscribeView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
