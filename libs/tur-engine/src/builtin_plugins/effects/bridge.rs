//! JS bridge for the `Opacity` / `Transform` effect elements.
//!
//! Builder pattern: `Opacity(props)` / `Transform(props)` return a chainable
//! builder terminated by `.build()`, which runs the terminal below with the
//! accumulated props.

use std::rc::Rc;

use boa_engine::{Context, JsResult, JsValue};

use crate::core::js_runtime::builder::builder_factory;
use crate::core::js_runtime::builder::setters::*;
use crate::core::js_runtime::builder::{BuilderMethod as M, BuilderTable};
use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_js_ctx, require_props_object, wrap_view,
};

static OPACITY_TABLE: BuilderTable = BuilderTable {
    methods: &[M::new("value", value), M::new("queryKey", queryKey)],
    child: true,
    children: false,
};

static TRANSFORM_TABLE: BuilderTable = BuilderTable {
    methods: &[
        M::new("scale", scale),
        M::new("scaleX", scaleX),
        M::new("scaleY", scaleY),
        M::new("rotate", rotate),
        M::new("translateX", translateX),
        M::new("translateY", translateY),
        M::new("alignment", alignment),
        M::new("queryKey", queryKey),
    ],
    child: true,
    children: false,
};

builder_factory!(tur_opacity_factory, tur_opacity, &OPACITY_TABLE);
builder_factory!(tur_transform_factory, tur_transform, &TRANSFORM_TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("Opacity", 2, tur_opacity_factory as Ptr),
        ("Transform", 2, tur_transform_factory as Ptr),
    ]
}

fn tur_opacity(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::OpacityView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_transform(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::TransformView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
