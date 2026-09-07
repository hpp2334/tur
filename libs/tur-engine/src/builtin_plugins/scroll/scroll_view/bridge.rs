//! JS bridge for the `ScrollView` element + scroll controller factory.
//!
//! Builder pattern: `ScrollView(props)` returns a chainable builder
//! terminated by `.build()`, which runs the terminal below with the
//! accumulated props (the required `child` fires there if missing).

use std::rc::Rc;

use boa_engine::class::Class;
use boa_engine::{Context, JsError, JsNativeError, JsResult, JsValue};

use crate::builtin_plugins::scroll::core::controller::ScrollController;
use crate::core::js_runtime::builder::builder_factory;
use crate::core::js_runtime::builder::setters::*;
use crate::core::js_runtime::builder::{BuilderMethod as M, BuilderTable};
use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_js_ctx, require_props_object, wrap_view,
};

static TABLE: BuilderTable = BuilderTable {
    methods: &[
        M::new("axis", axis),
        M::new("padding", padding),
        M::new("color", color),
        M::new("controller", controller),
        M::new("queryKey", queryKey),
    ],
    child: true,
    children: false,
};

builder_factory!(tur_scroll_view_factory, tur_scroll_view, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("ScrollView", 2, tur_scroll_view_factory as Ptr),
        (
            "createScrollController",
            2,
            tur_create_scroll_controller as Ptr,
        ),
    ]
}

fn tur_scroll_view(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::ScrollViewView::from_js(&props, context).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("missing required prop for ScrollViewView"))
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_create_scroll_controller(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let data = ScrollController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    Ok(ScrollController::from_data(data, context)?
        .upcast()
        .clone()
        .into())
}
