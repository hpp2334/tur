//! JS bridge for the `ReadableSubscribe` element.
//!
//! Builder pattern: `ReadableSubscribe(props)` returns a chainable builder
//! terminated by `.build()`; `.onUpdate$(mutation)` rides the rename rail
//! (`$` methods keep their key verbatim).

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
        M::new("readables", readables),
        M::renamed("onUpdate$", "onUpdate$", onUpdateMutation),
    ],
    child: true,
    children: false,
};

builder_factory!(
    tur_readable_subscribe_factory,
    tur_readable_subscribe,
    &TABLE
);

pub fn fns() -> Vec<FnEntry> {
    vec![(
        "ReadableSubscribe",
        2,
        tur_readable_subscribe_factory as Ptr,
    )]
}

fn tur_readable_subscribe(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::ReadableSubscribeView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
