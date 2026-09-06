//! JS bridge for the `Table` element.

use std::rc::Rc;

use boa_engine::{Context, JsError, JsNativeError, JsResult, JsValue};

use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_js_ctx, require_props_object, wrap_view,
};

pub fn fns() -> Vec<FnEntry> {
    vec![("Table", 2, tur_table as Ptr)]
}

fn tur_table(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::TableView::from_js(&props, context).ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message(
            "Table requires `columns` (non-empty array of { width?, flex?, minWidth? }), \
             `rows` (a readable) and `build` (a function)",
        ))
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}
