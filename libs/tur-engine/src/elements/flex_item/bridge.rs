//! JS bridge for the `Expanded` flex-item element.

use std::rc::Rc;

use boa_engine::{Context, JsError, JsNativeError, JsResult, JsValue};

use crate::core::bridge::helpers::{extract_ctx, require_props_object, wrap_view, FnEntry, Ptr};

pub(crate) fn fns() -> Vec<FnEntry> {
    vec![("Expanded", 2, tur_expanded as Ptr)]
}

fn tur_expanded(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::ExpandedView::from_js(&props, context).ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("missing required prop for ExpandedView"),
        )
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}
