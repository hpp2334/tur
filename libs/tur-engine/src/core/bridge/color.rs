use boa_engine::js_string;
use boa_engine::{Context, JsValue};
use tur_shared::Color;

pub(crate) fn extract_color(value: &JsValue, context: &mut Context) -> Option<Color> {
    let obj = value.as_object()?;
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
