//! JS bridge for the `Scrollbar` element.

use std::rc::Rc;

use boa_engine::{Context, JsResult, JsValue};

use crate::core::bridge::helpers::{extract_ctx, require_props_object, wrap_view, FnEntry, Ptr};

pub(crate) fn fns() -> Vec<FnEntry> {
    vec![("Scrollbar", 2, tur_scrollbar as Ptr)]
}

fn tur_scrollbar(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::ScrollbarView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
