//! JS bridge for the `Column` / `Row` flex elements.
//!
//! Builder pattern: `Column(props)` / `Row(props)` return a chainable
//! builder terminated by `.build()`, which runs the terminal below with the
//! accumulated props. The flex direction is fixed by the export the builder
//! was minted from (two terminals, one shared table).

use std::rc::Rc;

use boa_engine::{Context, JsResult, JsValue};

use crate::core::js_runtime::builder::builder_factory;
use crate::core::js_runtime::builder::setters::*;
use crate::core::js_runtime::builder::{BuilderMethod as M, BuilderTable};
use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_js_ctx, require_props_object, wrap_view,
};
use crate::core::layout::Axis;

static TABLE: BuilderTable = BuilderTable {
    methods: &[
        M::new("mainAlignment", mainAlignment),
        M::new("crossAlignment", crossAlignment),
        M::new("mainAxisSize", mainAxisSize),
        M::new("queryKey", queryKey),
    ],
    child: false,
    children: true,
};

builder_factory!(tur_column_factory, tur_column, &TABLE);
builder_factory!(tur_row_factory, tur_row, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("Column", 2, tur_column_factory as Ptr),
        ("Row", 2, tur_row_factory as Ptr),
    ]
}

fn tur_column(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::FlexView::from_js(Axis::Vertical, &props, context);
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_row(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::FlexView::from_js(Axis::Horizontal, &props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
