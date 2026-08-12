//! JS bridge for the `Stack` element.

use std::rc::Rc;

use boa_engine::{Context, JsResult, JsValue};

use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_js_ctx, require_props_object, wrap_view,
};

pub fn fns() -> Vec<FnEntry> {
    vec![("Stack", 2, tur_stack as Ptr)]
}

fn tur_stack(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::StackView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
