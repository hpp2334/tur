//! JS bridge for the `Column` / `Row` flex elements.

use std::rc::Rc;

use crate::core::layout::Axis;
use boa_engine::{Context, JsResult, JsValue};

use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_ctx, require_props_object, wrap_view,
};

pub fn fns() -> Vec<FnEntry> {
    vec![("Column", 2, tur_column as Ptr), ("Row", 2, tur_row as Ptr)]
}

fn tur_column(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::FlexView::from_js(Axis::Vertical, &props, context);
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_row(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::FlexView::from_js(Axis::Horizontal, &props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
