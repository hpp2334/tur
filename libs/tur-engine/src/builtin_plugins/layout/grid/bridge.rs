//! JS bridge for the `Grid` element.
//!
//! Builder pattern: `Grid(props)` returns a chainable builder terminated by
//! `.build()`, which runs the terminal below with the accumulated props
//! (`maxCrossAxisExtent` validation fires there).

use std::rc::Rc;

use boa_engine::{Context, JsError, JsNativeError, JsResult, JsValue};

use crate::core::js_runtime::builder::builder_factory;
use crate::core::js_runtime::builder::setters::*;
use crate::core::js_runtime::builder::{BuilderMethod as M, BuilderTable};
use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_js_ctx, require_props_object, wrap_view,
};

static TABLE: BuilderTable = BuilderTable {
    methods: &[
        M::new("maxCrossAxisExtent", maxCrossAxisExtent),
        M::new("childAspectRatio", childAspectRatio),
        M::new("mainAxisExtent", mainAxisExtent),
        M::new("crossAxisSpacing", crossAxisSpacing),
        M::new("mainAxisSpacing", mainAxisSpacing),
        M::new("queryKey", queryKey),
    ],
    child: false,
    children: true,
};

builder_factory!(tur_grid_factory, tur_grid, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![("Grid", 2, tur_grid_factory as Ptr)]
}

fn tur_grid(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::GridView::from_js(&props, context).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("missing required prop maxCrossAxisExtent for Grid"),
        )
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}
