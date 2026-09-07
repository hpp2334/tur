//! JS bridge for the `Positioned` element.
//!
//! Builder pattern: `Positioned(props)` returns a chainable builder
//! terminated by `.build()`, which runs the terminal below with the
//! accumulated props (edge-anchor validation fires there).

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
        M::new("left", left),
        M::new("top", top),
        M::new("right", right),
        M::new("bottom", bottom),
        M::new("width", width),
        M::new("height", height),
        M::new("queryKey", queryKey),
    ],
    child: true,
    children: false,
};

builder_factory!(tur_positioned_factory, tur_positioned, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![("Positioned", 2, tur_positioned_factory as Ptr)]
}

fn tur_positioned(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::PositionedView::from_js(&props, context).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("missing required prop for PositionedView"))
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}
