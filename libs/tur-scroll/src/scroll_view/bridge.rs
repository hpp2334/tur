//! JS bridge for the `ScrollView` element + scroll controller factory.

use std::rc::Rc;

use boa_engine::class::Class;
use boa_engine::{Context, JsError, JsNativeError, JsResult, JsValue};

use tur_engine::core::bridge::helpers::{extract_ctx, require_props_object, wrap_view, FnEntry, Ptr};
use crate::core::ScrollController;

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("ScrollView", 2, tur_scroll_view as Ptr),
        ("createScrollController", 2, tur_create_scroll_controller as Ptr),
    ]
}

fn tur_scroll_view(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::ScrollViewView::from_js(&props, context).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("missing required prop for ScrollViewView"),
        )
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_create_scroll_controller(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let data = ScrollController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    Ok(ScrollController::from_data(data, context)?.upcast().clone().into())
}
