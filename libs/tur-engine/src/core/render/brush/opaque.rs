use crate::core::edgy::value::{FromValue, Value, type_error};
use crate::core::render::brush::{Brush, Color};

// Native-KV decode: colors/brushes cross the reactive substrate as the
// packed `0xRRGGBBAA` u64 held plainly (`Value::Num`), or — legacy — as
// the wrapped engine Color/Brush handle (`Value::Opaque`, the identity
// lane). `0` packs no channels: the decode refuses it, so a seeded 0
// resolves to "absent" (the bound prop's clear), the same law the
// brush write rows spelled.
impl FromValue for Color {
    fn from_value(v: &Value) -> Result<Self, String> {
        if let Some(n) = v.as_num() {
            if n == 0.0 {
                return Err(type_error("a nonzero packed color (0 clears)"));
            }
            return Ok(Color::from_packed(n as u64));
        }
        v.as_opaque()
            .and_then(|o| o.downcast_ref::<Color>().copied())
            .ok_or_else(|| type_error("a packed color or a Color handle"))
    }
}

impl FromValue for Brush {
    fn from_value(v: &Value) -> Result<Self, String> {
        if let Some(n) = v.as_num() {
            if n == 0.0 {
                return Err(type_error("a nonzero packed color (0 clears)"));
            }
            return Ok(Brush::SolidColor(Color::from_packed(n as u64)));
        }
        let Some(o) = v.as_opaque() else {
            return Err(type_error("a packed color, a Brush or a Color handle"));
        };
        if let Some(b) = o.downcast_ref::<Brush>() {
            return Ok(b.clone());
        }
        o.downcast_ref::<Color>()
            .map(|c| Brush::SolidColor(*c))
            .ok_or_else(|| type_error("a packed color, a Brush or a Color handle"))
    }
}
