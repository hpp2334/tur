//! JS bridge for the `LazyList` element + its controller factory.

use std::rc::Rc;

use boa_engine::class::Class;
use boa_engine::{Context, JsError, JsNativeError, JsResult, JsValue};

use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_js_ctx, require_props_object, wrap_view,
};

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("LazyList", 2, tur_lazy_list as Ptr),
        (
            "createLazyListController",
            2,
            tur_create_lazy_list_controller as Ptr,
        ),
    ]
}

fn tur_lazy_list(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::LazyListView::from_js(&props, context).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("missing required prop for LazyListView"))
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_create_lazy_list_controller(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let data =
        super::LazyListController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    Ok(super::LazyListController::from_data(data, context)?
        .upcast()
        .clone()
        .into())
}
