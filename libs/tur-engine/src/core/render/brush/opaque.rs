use crate::core::render::brush::{Brush, Color};
use boa_engine::JsValue;
use boa_gc::{Finalize, Trace};

use crate::core::js_runtime::js_value::{FromJs, type_error};

#[derive(Debug, Clone, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct ColorOpaque(pub Color);

#[derive(Debug, Clone, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct BrushOpaque(pub Brush);

impl FromJs for Color {
    fn from_js(v: &JsValue) -> Result<Self, boa_engine::JsError> {
        v.as_object()
            .and_then(|obj| obj.downcast_ref::<ColorOpaque>().map(|c| c.0))
            .ok_or_else(|| type_error("a Color handle"))
    }
}

impl FromJs for Brush {
    fn from_js(v: &JsValue) -> Result<Self, boa_engine::JsError> {
        let Some(obj) = v.as_object() else {
            return Err(type_error("a Brush or Color handle"));
        };
        if let Some(b) = obj.downcast_ref::<BrushOpaque>() {
            return Ok(b.0.clone());
        }
        if let Some(c) = obj.downcast_ref::<ColorOpaque>() {
            return Ok(Brush::SolidColor(c.0));
        }
        Err(type_error("a Brush or Color handle"))
    }
}
