//! JS bridge for the `Switch` element.

use std::rc::Rc;

use boa_engine::{Context, JsResult, JsValue};

use crate::core::js_runtime::helpers::{extract_ctx, require_props_object, wrap_view, FnEntry, Ptr};

pub fn fns() -> Vec<FnEntry> {
    vec![("Switch", 2, tur_switch as Ptr)]
}

fn tur_switch(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::SwitchView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
