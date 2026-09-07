//! JS bridge for the `Condition` element.
//!
//! Builder pattern: `Condition(props)` returns a chainable builder
//! terminated by `.build()`. `.child(fn)` / `.elseChild(fn)` take the
//! branch thunks (`() => Element`).

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
        M::new("condition", condition),
        M::new("elseChild", elseChild),
        M::new("queryKey", queryKey),
    ],
    child: true,
    children: false,
};

builder_factory!(tur_condition_factory, tur_condition, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![("Condition", 2, tur_condition_factory as Ptr)]
}

fn tur_condition(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::ConditionView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
