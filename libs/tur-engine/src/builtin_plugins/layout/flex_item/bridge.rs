//! JS bridge for the `Expanded` flex-item element.
//!
//! Builder pattern: `Expanded(props)` returns a chainable builder terminated
//! by `.build()`, which runs the terminal below with the accumulated props.

use std::rc::Rc;

use boa_engine::{Context, JsError, JsNativeError, JsResult, JsValue};

use crate::core::js_runtime::builder::builder_factory;
use crate::core::js_runtime::builder::setters::*;
use crate::core::js_runtime::builder::{BuilderMethod as M, BuilderTable};
use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_js_ctx, require_props_object, wrap_view,
};

static TABLE: BuilderTable = BuilderTable {
    methods: &[M::new("flex", flex), M::new("queryKey", queryKey)],
    child: true,
    children: false,
};

builder_factory!(tur_expanded_factory, tur_expanded, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![("Expanded", 2, tur_expanded_factory as Ptr)]
}

fn tur_expanded(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::ExpandedView::from_js(&props, context).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("missing required prop for ExpandedView"))
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}
