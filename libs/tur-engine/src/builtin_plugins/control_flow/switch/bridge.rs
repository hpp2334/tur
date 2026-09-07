//! JS bridge for the `Switch` element.
//!
//! Builder pattern: `Switch(props)` returns a chainable builder terminated
//! by `.build()`. `.cases([...])` takes the `{ key, child: () => Element }`
//! entries; `.fallback(fn)` the fallback thunk.

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
        M::new("value", value),
        M::new("cases", cases),
        M::new("fallback", fallback),
        M::new("queryKey", queryKey),
    ],
    child: false,
    children: false,
};

builder_factory!(tur_switch_factory, tur_switch, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![("Switch", 2, tur_switch_factory as Ptr)]
}

fn tur_switch(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::SwitchView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
