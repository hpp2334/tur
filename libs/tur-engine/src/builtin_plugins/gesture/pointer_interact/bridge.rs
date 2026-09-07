//! JS bridge for the `PointerInteract` element.
//!
//! Builder pattern: `PointerInteract(props)` returns a chainable builder
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
        M::new("onClick", onClick),
        M::new("onPointerDown", onPointerDown),
        M::new("onPointerMove", onPointerMove),
        M::new("onPointerUp", onPointerUp),
        M::new("onContextMenu", onContextMenu),
        M::new("queryKey", queryKey),
    ],
    child: true,
    children: false,
};

builder_factory!(tur_pointer_interact_factory, tur_pointer_interact, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![("PointerInteract", 2, tur_pointer_interact_factory as Ptr)]
}

fn tur_pointer_interact(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::PointerInteractView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
