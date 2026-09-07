//! JS bridge for the `Input` element + text editing controllers.
//!
//! Builder pattern: `Input(props)` returns a chainable builder terminated by
//! `.build()`, which runs the terminal below with the accumulated props.

use std::rc::Rc;

use boa_engine::class::Class;
use boa_engine::{Context, JsResult, JsValue};

use crate::builtin_plugins::text::controller::{TextEditingController, UndoController};
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
        M::new("controller", controller),
        M::new("undoController", undoController),
        M::new("placeholder", placeholder),
        M::new("color", color),
        M::new("placeholderColor", placeholderColor),
        M::new("cursorColor", cursorColor),
        M::new("fontSize", fontSize),
        M::new("fontFamily", fontFamily),
        M::new("fontWeight", fontWeight),
        M::new("multiline", multiline),
        M::new("obscureText", obscureText),
        M::new("obscuringCharacter", obscuringCharacter),
        M::new("onContextMenu", onContextMenu),
        M::new("queryKey", queryKey),
    ],
    child: false,
    children: false,
};

builder_factory!(tur_input_factory, tur_input, &TABLE);

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("Input", 2, tur_input_factory as Ptr),
        (
            "createTextEditingController",
            2,
            tur_create_text_editing_controller as Ptr,
        ),
        ("createUndoController", 2, tur_create_undo_controller as Ptr),
    ]
}

fn tur_input(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::InputView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_create_text_editing_controller(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let data = TextEditingController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    Ok(TextEditingController::from_data(data, context)?
        .upcast()
        .clone()
        .into())
}

fn tur_create_undo_controller(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let data = UndoController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    Ok(UndoController::from_data(data, context)?
        .upcast()
        .clone()
        .into())
}
