use boa_engine::js_string;
use boa_engine::{Context, JsValue};
use tur_shared::{Brush, Color, GradientStop};

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

fn extract_offset_pair(
    obj: &boa_engine::object::JsObject,
    key: &str,
    context: &mut Context,
) -> Option<(f64, f64)> {
    let arr_val = obj.get(js_string!(key), context).ok()?;
    let arr_obj = arr_val.as_object()?;
    let arr = boa_engine::object::builtins::JsArray::from_object(arr_obj.clone()).ok()?;
    let x = arr.at(0, context).ok()?.as_number()?;
    let y = arr.at(1, context).ok()?.as_number()?;
    Some((x, y))
}

fn extract_stops(
    obj: &boa_engine::object::JsObject,
    context: &mut Context,
) -> Option<Vec<GradientStop>> {
    let stops_val = obj.get(js_string!("stops"), context).ok()?;
    let stops_obj = stops_val.as_object()?;
    let stops_arr =
        boa_engine::object::builtins::JsArray::from_object(stops_obj.clone()).ok()?;
    let len = stops_arr.length(context).ok()? as usize;
    let mut stops = Vec::with_capacity(len);
    for i in 0..len {
        let stop_val = stops_arr.at(i as i64, context).ok()?;
        let stop_obj = stop_val.as_object()?;
        let offset = stop_obj
            .get(js_string!("offset"), context)
            .ok()?
            .as_number()? as f32;
        let color = extract_color(&stop_val, context)?;
        stops.push(GradientStop { offset, color });
    }
    Some(stops)
}

pub(crate) fn extract_brush(value: &JsValue, context: &mut Context) -> Option<Brush> {
    let obj = value.as_object()?;
    let type_val = obj.get(js_string!("type"), context).ok()?;
    let type_str = type_val.as_string()?.to_std_string_escaped();

    match type_str.as_str() {
        "solid" => {
            let color = extract_color(value, context)?;
            Some(Brush::SolidColor(color))
        }
        "linear" => {
            let start = extract_offset_pair(&obj, "start", context)?;
            let end = extract_offset_pair(&obj, "end", context)?;
            let stops = extract_stops(&obj, context)?;
            if stops.is_empty() {
                return None;
            }
            Some(Brush::LinearGradient {
                start,
                end,
                stops,
            })
        }
        _ => None,
    }
}
