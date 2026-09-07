//! JS bridge for the `Each` element.
//!
//! Builder pattern: `Each(props)` returns a chainable builder terminated by
//! `.build()`. The item factory — the `build: (item, index) => Element`
//! prop — is exposed as `.itemBuilder(fn)` (the prop key `build` would
//! collide with the builder's terminal `.build()`).

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
        M::new("items", items),
        M::renamed("itemBuilder", "build", itemBuilder),
        M::new("queryKey", queryKey),
    ],
    child: false,
    children: false,
};

builder_factory!(tur_each_factory, tur_each, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![("Each", 2, tur_each_factory as Ptr)]
}

fn tur_each(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::EachView::from_js(&props, context).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("missing required prop for EachView"))
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}
