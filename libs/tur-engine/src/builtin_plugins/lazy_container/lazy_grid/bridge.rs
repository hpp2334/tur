//! JS bridge for the `LazyGrid` element + its controller factory.
//!
//! Builder pattern: `LazyGrid(props)` returns a chainable builder terminated
//! by `.build()`, which runs the terminal below with the accumulated props
//! (required-prop validation fires there).

use std::rc::Rc;

use boa_engine::class::Class;
use boa_engine::{Context, JsError, JsNativeError, JsResult, JsValue};

use crate::core::js_runtime::builder::builder_factory;
use crate::core::js_runtime::builder::setters::*;
use crate::core::js_runtime::builder::{BuilderMethod as M, BuilderTable};
use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_js_ctx, require_props_object, wrap_view,
};

static TABLE: BuilderTable = BuilderTable {
    methods: &[
        M::new("itemCount", itemCount),
        M::new("maxCrossAxisExtent", maxCrossAxisExtent),
        M::new("builder", builder),
        M::new("axis", axis),
        M::new("overscan", overscan),
        M::new("childAspectRatio", childAspectRatio),
        M::new("mainAxisExtent", mainAxisExtent),
        M::new("crossAxisSpacing", crossAxisSpacing),
        M::new("mainAxisSpacing", mainAxisSpacing),
        M::new("queryKey", queryKey),
    ],
    child: false,
    children: false,
};

builder_factory!(tur_lazy_grid_factory, tur_lazy_grid, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("LazyGrid", 2, tur_lazy_grid_factory as Ptr),
        (
            "createLazyGridController",
            2,
            tur_create_lazy_grid_controller as Ptr,
        ),
    ]
}

fn tur_lazy_grid(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::LazyGridView::from_js(&props, context).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(
            "missing required prop for LazyGridView (itemCount, maxCrossAxisExtent, builder)",
        ))
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_create_lazy_grid_controller(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let data =
        super::LazyGridController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    Ok(super::LazyGridController::from_data(data, context)?
        .upcast()
        .clone()
        .into())
}
