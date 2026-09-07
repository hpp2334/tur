//! JS bridge for the `Stack` element.
//!
//! Builder pattern: `Stack(props)` returns a chainable builder terminated
//! by `.build()`, which runs the terminal below with the accumulated props.

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
        M::new("fit", fit),
        M::new("alignment", alignment),
        M::new("queryKey", queryKey),
    ],
    child: false,
    children: true,
};

builder_factory!(tur_stack_factory, tur_stack, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![("Stack", 2, tur_stack_factory as Ptr)]
}

fn tur_stack(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::StackView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
