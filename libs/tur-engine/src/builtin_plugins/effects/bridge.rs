//! JS bridge for the `Opacity` / `Transform` effect elements.

use std::rc::Rc;

use boa_engine::{Context, JsResult, JsValue};

use crate::core::js_runtime::helpers::{
    extract_ctx, require_props_object, wrap_view, FnEntry, Ptr,
};

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("Opacity", 2, tur_opacity as Ptr),
        ("Transform", 2, tur_transform as Ptr),
    ]
}

fn tur_opacity(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::OpacityView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_transform(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::TransformView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
