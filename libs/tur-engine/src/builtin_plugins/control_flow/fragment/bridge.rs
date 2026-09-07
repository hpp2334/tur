//! JS bridge for the `Fragment` element.
//!
//! Builder pattern: `Fragment(props)` returns a chainable builder terminated
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
    methods: &[M::new("queryKey", queryKey)],
    child: false,
    children: true,
};

builder_factory!(tur_fragment_factory, tur_fragment, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![("Fragment", 2, tur_fragment_factory as Ptr)]
}

fn tur_fragment(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::FragmentView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
