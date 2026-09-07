//! JS bridge for the `MouseRegion` element.
//!
//! Builder pattern: `MouseRegion(props)` returns a chainable builder
//! terminated by `.build()`, which runs the terminal below with the
//! accumulated props.

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
        M::new("behavior", behavior),
        M::new("cursor", cursor),
        M::new("onEnter", onEnter),
        M::new("onExit", onExit),
        M::new("queryKey", queryKey),
    ],
    child: true,
    children: false,
};

builder_factory!(tur_mouse_region_factory, tur_mouse_region, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![("MouseRegion", 2, tur_mouse_region_factory as Ptr)]
}

fn tur_mouse_region(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::MouseRegionView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
