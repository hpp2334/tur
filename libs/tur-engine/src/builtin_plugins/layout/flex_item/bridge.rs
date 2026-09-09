//! JS bridge for the flex-item elements (`Expanded` / `Flexible`).
//!
//! Builder pattern: `Expanded(props)` / `Flexible(props)` return a chainable
//! builder terminated by `.build()`, which runs the terminals below with the
//! accumulated props.
//!
//! Flutter hierarchy mirrored: `Flexible` is the base (default
//! `FlexFit.loose` — the child may be at most its slot, smaller allowed) and
//! `Expanded` is `Flexible` with `FlexFit.tight` (the child is forced to
//! fill its slot; no `fit` setter is exposed, exactly like Flutter's
//! `Expanded`).

use std::rc::Rc;

use boa_engine::{Context, JsError, JsNativeError, JsResult, JsValue};

use crate::core::js_runtime::builder::builder_factory;
use crate::core::js_runtime::builder::setters::*;
use crate::core::js_runtime::builder::{BuilderMethod as M, BuilderTable};
use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_js_ctx, require_props_object, wrap_view,
};
use crate::core::layout::FlexFit;

static EXPANDED_TABLE: BuilderTable = BuilderTable {
    methods: &[M::new("flex", flex), M::new("queryKey", queryKey)],
    child: true,
    children: false,
};

static FLEXIBLE_TABLE: BuilderTable = BuilderTable {
    methods: &[
        M::new("flex", flex),
        M::new("fit", fit),
        M::new("queryKey", queryKey),
    ],
    child: true,
    children: false,
};

builder_factory!(tur_expanded_factory, tur_expanded, &EXPANDED_TABLE);
builder_factory!(tur_flexible_factory, tur_flexible, &FLEXIBLE_TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("Expanded", 2, tur_expanded_factory as Ptr),
        ("Flexible", 2, tur_flexible_factory as Ptr),
    ]
}

fn tur_expanded(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::FlexibleView::from_js(&props, context, FlexFit::Tight).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("missing required prop `child` for Expanded"),
        )
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_flexible(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::FlexibleView::from_js(&props, context, FlexFit::Loose).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("missing required prop `child` for Flexible"),
        )
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}
