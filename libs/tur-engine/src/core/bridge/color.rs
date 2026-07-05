use boa_engine::js_string;
use boa_engine::{Context, JsValue};
use boa_gc::{Finalize, Trace};
use tur_shared::{Brush, Color};

use crate::core::view::val::PropValue;

#[derive(Debug, Clone, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct ColorOpaque(pub Color);

#[derive(Debug, Clone, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct BrushOpaque(pub Brush);

impl PropValue for Color {
    fn from_js(v: &JsValue) -> Option<Self> {
        v.as_object()?.downcast_ref::<ColorOpaque>().map(|c| c.0)
    }
}

impl PropValue for Brush {
    fn from_js(v: &JsValue) -> Option<Self> {
        let obj = v.as_object()?;
        if let Some(b) = obj.downcast_ref::<BrushOpaque>() {
            return Some(b.0.clone());
        }
        if let Some(c) = obj.downcast_ref::<ColorOpaque>() {
            return Some(Brush::SolidColor(c.0));
        }
        None
    }
}

pub fn extract_color(value: &JsValue, context: &mut Context) -> Option<Color> {
    let obj = value.as_object()?;
    if let Some(c) = obj.downcast_ref::<ColorOpaque>() {
        return Some(c.0);
    }
    let r = obj.get(js_string!("r"), context).ok()?.as_number()? as u8;
    let g = obj.get(js_string!("g"), context).ok()?.as_number()? as u8;
    let b = obj.get(js_string!("b"), context).ok()?.as_number()? as u8;
    let a = obj
        .get(js_string!("a"), context)
        .ok()
        .and_then(|v| v.as_number())
        .unwrap_or(255.0) as u8;
    Some(Color::rgba(r, g, b, a))
}
