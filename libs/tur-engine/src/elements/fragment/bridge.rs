//! JS bridge for the `Fragment` element.

use std::rc::Rc;

use boa_engine::{Context, JsResult, JsValue};

use crate::core::bridge::helpers::{extract_ctx, require_props_object, wrap_view, FnEntry, Ptr};

pub(crate) fn fns() -> Vec<FnEntry> {
    vec![("Fragment", 2, tur_fragment as Ptr)]
}

fn tur_fragment(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::FragmentView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
