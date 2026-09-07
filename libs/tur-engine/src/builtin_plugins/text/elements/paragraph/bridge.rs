//! JS bridge for the `Text` element.
//!
//! Builder pattern: `Text(props)` returns a chainable builder terminated by
//! `.build()`, which runs the terminal below with the accumulated props.

use std::rc::Rc;

use boa_engine::{Context, JsResult, JsValue};

use crate::core::js_runtime::builder::builder_factory;
use crate::core::js_runtime::builder::setters::*;
use crate::core::js_runtime::builder::{BuilderMethod as M, BuilderTable};
use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_js_ctx, require_props_object, wrap_view,
};

static TABLE: BuilderTable = BuilderTable {
    methods: &[
        M::new("text", text),
        M::new("fontSize", fontSize),
        M::new("fontWeight", fontWeight),
        M::new("color", color),
        M::new("spans", spans),
        M::new("maxLines", maxLines),
        M::new("overflow", overflow),
        M::new("selectable", selectable),
        M::new("onSelectionChange", onSelectionChange),
        M::new("queryKey", queryKey),
    ],
    child: false,
    children: false,
};

builder_factory!(tur_text_factory, tur_text, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![("Text", 2, tur_text_factory as Ptr)]
}

fn tur_text(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::TextView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
