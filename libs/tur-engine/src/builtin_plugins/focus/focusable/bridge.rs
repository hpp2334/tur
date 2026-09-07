//! JS bridge for the `Focusable` element + `requestFocus`.
//!
//! Builder pattern: `Focusable(props)` returns a chainable builder
//! terminated by `.build()`, which runs the terminal below with the
//! accumulated props.

use std::rc::Rc;

use boa_engine::{Context, JsArgs, JsNativeError, JsResult, JsValue};

use crate::core::js_runtime::BoaOpaque;
use crate::core::js_runtime::builder::builder_factory;
use crate::core::js_runtime::builder::setters::*;
use crate::core::js_runtime::builder::{BuilderMethod as M, BuilderTable};
use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, TurNodeHandle, extract_js_ctx, require_props_object, wrap_view,
};

static TABLE: BuilderTable = BuilderTable {
    methods: &[
        M::new("onKeyDown", onKeyDown),
        M::new("onKeyUp", onKeyUp),
        M::new("onFocus", onFocus),
        M::new("onBlur", onBlur),
    ],
    child: true,
    children: false,
};

builder_factory!(tur_focusable_factory, tur_focusable, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("Focusable", 2, tur_focusable_factory as Ptr),
        ("requestFocus", 2, tur_request_focus as Ptr),
    ]
}

fn tur_focusable(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::FocusableView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_request_focus(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let obj = args
        .get_or_undefined(1)
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("expected TurNodeHandle"))?;
    let handle = BoaOpaque::<TurNodeHandle>::wrap(&obj)
        .ok_or_else(|| JsNativeError::typ().with_message("expected TurNodeHandle"))?;
    let mut focus = js_ctx.focus_manager.borrow_mut();
    focus.set_focus(handle.id);
    Ok(JsValue::undefined())
}
