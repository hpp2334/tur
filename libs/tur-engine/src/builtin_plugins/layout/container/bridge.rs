//! JS bridge for the `Container` element (+ `SizedBox` alias).

use std::rc::Rc;

use boa_engine::{Context, JsResult, JsValue};

use crate::core::js_runtime::helpers::{
    FnEntry, Ptr, extract_ctx, require_props_object, wrap_view,
};

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("Container", 2, tur_container as Ptr),
        // `SizedBox` is a width/height-only `Container` — same native fn,
        // exported under an alias so JS callers write `SizedBox({width,height})`.
        ("SizedBox", 2, tur_container as Ptr),
    ]
}

fn tur_container(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::ContainerView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}
