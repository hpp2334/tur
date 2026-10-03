use crate::core::edgy::value::{FromValue, Value, type_error};
use crate::core::render::brush::{Brush, Color};

// Native-KV decode: colors/brushes cross the reactive substrate opaquely
// (the wrap keeps the handle's identity; see `edgy::Value`).
impl FromValue for Color {
    fn from_value(v: &Value) -> Result<Self, String> {
        v.as_opaque()
            .and_then(|o| o.downcast_ref::<Color>().copied())
            .ok_or_else(|| type_error("a Color handle"))
    }
}

impl FromValue for Brush {
    fn from_value(v: &Value) -> Result<Self, String> {
        let Some(o) = v.as_opaque() else {
            return Err(type_error("a Brush or Color handle"));
        };
        if let Some(b) = o.downcast_ref::<Brush>() {
            return Ok(b.clone());
        }
        o.downcast_ref::<Color>()
            .map(|c| Brush::SolidColor(*c))
            .ok_or_else(|| type_error("a Brush or Color handle"))
    }
}
