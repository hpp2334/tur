//! JS bridge for the `Table` element.
//!
//! Builder pattern: `Table(props)` returns a chainable builder terminated by
//! `.build()`, which runs the terminal below with the accumulated props.
//! The `build` / `buildHeader` row/header callbacks are exposed as
//! `.rowBuilder(fn)` / `.headerBuilder(fn)` — the prop key `build` would
//! collide with the builder's terminal `.build()`.

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
        M::new("columns", columns),
        M::new("rows", rows),
        M::renamed("rowBuilder", "build", rowBuilder),
        M::renamed("headerBuilder", "buildHeader", headerBuilder),
        M::new("headerExtent", headerExtent),
        M::new("rowExtent", rowExtent),
        M::new("rowSpacing", rowSpacing),
        M::new("stripeColor", stripeColor),
        M::new("dividerColor", dividerColor),
        M::new("dividerThickness", dividerThickness),
        M::new("queryKey", queryKey),
    ],
    child: false,
    children: false,
};

builder_factory!(tur_table_factory, tur_table, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![("Table", 2, tur_table_factory as Ptr)]
}

fn tur_table(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::TableView::from_js(&props, context).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(
            "Table requires `columns` (non-empty array of { width?, flex?, minWidth? }), \
             `rows` (a readable) and `build` (a function)",
        ))
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}
