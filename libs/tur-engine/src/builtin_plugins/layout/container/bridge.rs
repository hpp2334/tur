//! JS bridge for the `Container` element (+ `SizedBox` alias).
//!
//! Builder pattern: `Container(props)` returns a chainable builder
//! (`.width(…)` / `.padding(…)` / `.children([…])` / …) terminated by
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
        M::new("width", width),
        M::new("height", height),
        M::new("padding", padding),
        M::new("color", color),
        M::new("borderColor", borderColor),
        M::new("borderWidth", borderWidth),
        M::new("borderRadius", borderRadius),
        M::new("borderPosition", borderPosition),
        M::new("clipBehavior", clipBehavior),
        M::new("shadowColor", shadowColor),
        M::new("shadowBlur", shadowBlur),
        M::new("shadowOffset", shadowOffset),
        M::new("alignment", alignment),
        M::new("queryKey", queryKey),
    ],
    child: false,
    children: true,
};

builder_factory!(tur_container_factory, tur_container, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("Container", 2, tur_container_factory as Ptr),
        // `SizedBox` is a width/height-only `Container` — same builder
        // table + terminal, exported under an alias.
        ("SizedBox", 2, tur_container_factory as Ptr),
    ]
}

fn tur_container(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::ContainerView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
