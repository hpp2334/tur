//! JS bridge for the `Input` element + text editing controllers.

use std::rc::Rc;

use boa_engine::class::Class;
use boa_engine::{Context, JsResult, JsValue};

use crate::core::bridge::helpers::{extract_ctx, require_props_object, wrap_view, FnEntry, Ptr};
use crate::core::text::controller::{TextEditingController, UndoController};

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("Input", 2, tur_input as Ptr),
        ("createTextEditingController", 2, tur_create_text_editing_controller as Ptr),
        ("createUndoController", 2, tur_create_undo_controller as Ptr),
    ]
}

fn tur_input(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::InputView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_create_text_editing_controller(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let data = TextEditingController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    Ok(TextEditingController::from_data(data, context)?.upcast().clone().into())
}

fn tur_create_undo_controller(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let data = UndoController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    Ok(UndoController::from_data(data, context)?.upcast().clone().into())
}
