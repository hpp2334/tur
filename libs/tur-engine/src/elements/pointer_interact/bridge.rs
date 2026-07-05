//! JS bridge for the `PointerInteract` element.

use std::rc::Rc;

use boa_engine::{Context, JsResult, JsValue};

use crate::core::bridge::helpers::{extract_ctx, require_props_object, wrap_view, FnEntry, Ptr};

pub(crate) fn fns() -> Vec<FnEntry> {
    vec![("PointerInteract", 2, tur_pointer_interact as Ptr)]
}

fn tur_pointer_interact(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::PointerInteractView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
