use crate::core::edgy::value::{FromValue, Value, type_error};
use crate::core::render::brush::{Brush, Color};
use boa_engine::JsValue;
use boa_gc::{Finalize, Trace};

use crate::core::js_runtime::js_value::FromJs;

#[derive(Debug, Clone, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct ColorOpaque(pub Color);

#[derive(Debug, Clone, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct BrushOpaque(pub Brush);

fn decode_color_opaque(v: &JsValue) -> Option<Color> {
    v.as_object()
        .and_then(|obj| obj.downcast_ref::<ColorOpaque>().map(|c| c.0))
}

fn decode_brush_opaque(v: &JsValue) -> Option<Brush> {
    let obj = v.as_object()?;
    if let Some(b) = obj.downcast_ref::<BrushOpaque>() {
        return Some(b.0.clone());
    }
    obj.downcast_ref::<ColorOpaque>().map(|c| Brush::SolidColor(c.0))
}

impl FromJs for Color {
    fn from_js(v: &JsValue) -> Result<Self, boa_engine::JsError> {
        decode_color_opaque(v).ok_or_else(|| type_error("a Color handle"))
    }
}

impl FromJs for Brush {
    fn from_js(v: &JsValue) -> Result<Self, boa_engine::JsError> {
        decode_brush_opaque(v).ok_or_else(|| type_error("a Brush or Color handle"))
    }
}

// Native-KV decode: colors/brushes cross the reactive substrate opaquely
// (the JS-boundary wrap keeps the handle's identity; see `edgy::Value`).
impl FromValue for Color {
    fn from_value(v: &Value) -> Result<Self, boa_engine::JsError> {
        v.as_opaque()
            .and_then(decode_color_opaque)
            .ok_or_else(|| type_error("a Color handle"))
    }
}

impl FromValue for Brush {
    fn from_value(v: &Value) -> Result<Self, boa_engine::JsError> {
        v.as_opaque()
            .and_then(decode_brush_opaque)
            .ok_or_else(|| type_error("a Brush or Color handle"))
    }
}
