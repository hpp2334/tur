use boa_engine::js_string;
use boa_engine::{Context, JsArgs, JsValue};
use boa_gc::{Finalize, Trace};
use tur_shared::{Brush, Color, GradientStop};

use crate::core::bridge::BoaOpaque;
use crate::core::view::val::PropValue;

// ---------------------------------------------------------------------------
// Opaque wrappers for tur-shared color/brush types so they can be stored
// inside boa JS objects (NativeObject).  tur-shared cannot depend on boa, so
// we wrap here.  Construction happens in the bridge functions below; decoding
// happens via the `PropValue` impls.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct ColorOpaque(pub Color);

#[derive(Debug, Clone, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct BrushOpaque(pub Brush);

// --- PropValue impls (no boa Context needed to decode) ---

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
        // A Color is implicitly a solid Brush.
        if let Some(c) = obj.downcast_ref::<ColorOpaque>() {
            return Some(Brush::SolidColor(c.0));
        }
        None
    }
}

/// Bridge function `createColor(r, g, b, a)` → JS opaque wrapping `Color`.
/// Called by the TS `Color` class so that Rust can read the value via
/// `downcast_ref::<ColorOpaque>()` without needing a boa `Context`.
pub(crate) fn tur_create_color(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> boa_engine::JsResult<JsValue> {
    // `args[0]` is the bound bridge ctx (prepended by `bound_native` when the
    // `builtin:tur/core` module is built); the real RGBA args follow at [1..].
    let r = args.get_or_undefined(1).as_number().unwrap_or(0.0) as u8;
    let g = args.get_or_undefined(2).as_number().unwrap_or(0.0) as u8;
    let b = args.get_or_undefined(3).as_number().unwrap_or(0.0) as u8;
    let a = args.get_or_undefined(4).as_number().unwrap_or(255.0) as u8;
    let opaque = BoaOpaque::new(ColorOpaque(Color::rgba(r, g, b, a)), context);
    Ok(opaque.object().clone().into())
}

/// Bridge function `colorLerp(colorA, colorB, t)` → new `Color` opaque at
/// the interpolated position. Backs the JS-facing `ColorTween.lerp`. Mirrors
/// Flutter's `Color.lerp` (which `ColorTween` delegates to).
pub(crate) fn tur_color_lerp(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> boa_engine::JsResult<JsValue> {
    // `args[0]` is the bound bridge ctx (see `tur_create_color`); real args at [1..].
    let a = extract_color(args.get_or_undefined(1), context)
        .ok_or_else(|| boa_engine::JsNativeError::typ().with_message("colorLerp: `begin` must be a Color"))?;
    let b = extract_color(args.get_or_undefined(2), context)
        .ok_or_else(|| boa_engine::JsNativeError::typ().with_message("colorLerp: `end` must be a Color"))?;
    let t = args.get_or_undefined(3).as_number().unwrap_or(0.0);
    let out = Color::lerp(a, b, t);
    let opaque = BoaOpaque::new(ColorOpaque(out), context);
    Ok(opaque.object().clone().into())
}

/// Bridge function `createLinearGradient(startX, startY, endX, endY, stops)`
/// where stops is an array of `{offset, r, g, b, a}`. Returns JS opaque
/// wrapping `Brush::LinearGradient`.
pub(crate) fn tur_create_linear_gradient(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> boa_engine::JsResult<JsValue> {
    // `args[0]` is the bound bridge ctx (see `tur_create_color`); real args at [1..].
    let start = (
        args.get_or_undefined(1).as_number().unwrap_or(0.0),
        args.get_or_undefined(2).as_number().unwrap_or(0.0),
    );
    let end = (
        args.get_or_undefined(3).as_number().unwrap_or(0.0),
        args.get_or_undefined(4).as_number().unwrap_or(0.0),
    );
    let mut stops: Vec<GradientStop> = Vec::new();
    if let Some(stops_val) = args.get_or_undefined(5).as_object() {
        if let Ok(arr) = boa_engine::object::builtins::JsArray::from_object(stops_val.clone()) {
            let len = arr.length(context).unwrap_or(0);
            for i in 0..len {
                if let Ok(stop_val) = arr.at(i as i64, context) {
                    let stop_obj = match stop_val.as_object() {
                        Some(o) => o,
                        None => continue,
                    };
                    let offset = stop_obj
                        .get(js_string!("offset"), context)
                        .ok()
                        .and_then(|v| v.as_number())
                        .unwrap_or(0.0) as f32;
                    let r = stop_obj.get(js_string!("r"), context).ok().and_then(|v| v.as_number()).unwrap_or(0.0) as u8;
                    let g = stop_obj.get(js_string!("g"), context).ok().and_then(|v| v.as_number()).unwrap_or(0.0) as u8;
                    let b = stop_obj.get(js_string!("b"), context).ok().and_then(|v| v.as_number()).unwrap_or(0.0) as u8;
                    let a = stop_obj.get(js_string!("a"), context).ok().and_then(|v| v.as_number()).unwrap_or(255.0) as u8;
                    stops.push(GradientStop { offset, color: Color::rgba(r, g, b, a) });
                }
            }
        }
    }
    let brush = Brush::LinearGradient { start, end, stops };
    let opaque = BoaOpaque::new(BrushOpaque(brush), context);
    Ok(opaque.object().clone().into())
}

/// `Color.rgb(r, g, b)` — bound method on the native `Color` const-object.
/// `args = [ctx, r, g, b]`; forwards to `tur_create_color` with `a = 255`.
pub(crate) fn tur_color_rgb(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> boa_engine::JsResult<JsValue> {
    let full: Vec<JsValue> = vec![
        args.get_or_undefined(0).clone(),
        args.get_or_undefined(1).clone(),
        args.get_or_undefined(2).clone(),
        args.get_or_undefined(3).clone(),
        JsValue::from(255),
    ];
    tur_create_color(_this, &full, context)
}

/// `Color.rgba(r, g, b, a)` — bound method. Same arg layout as
/// `tur_create_color` (`[ctx, r, g, b, a]`), so it forwards verbatim.
pub(crate) fn tur_color_rgba(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> boa_engine::JsResult<JsValue> {
    tur_create_color(_this, args, context)
}

/// `Color.hex("#RRGGBB[AA]" | "#RGB")` — bound method. Parses the hex string
/// and forwards to `tur_create_color`.
pub(crate) fn tur_color_hex(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> boa_engine::JsResult<JsValue> {
    let hex = args
        .get_or_undefined(1)
        .as_string()
        .ok_or_else(|| {
            boa_engine::JsError::from(
                boa_engine::JsNativeError::typ()
                    .with_message("Color.hex: expected a hex color string"),
            )
        })?
        .to_std_string_escaped();
    let (r, g, b, a) = parse_hex_color(&hex).ok_or_else(|| {
        boa_engine::JsError::from(
            boa_engine::JsNativeError::typ()
                .with_message(format!("Color.hex: invalid hex color: {hex}")),
        )
    })?;
    let full = [
        args.get_or_undefined(0).clone(),
        JsValue::from(r),
        JsValue::from(g),
        JsValue::from(b),
        JsValue::from(a),
    ];
    tur_create_color(_this, &full, context)
}

/// Parse a CSS-style hex color (`"#RGB"`, `"#RRGGBB"`, `"#RRGGBBAA"`, with or
/// without the leading `#`) into `(r, g, b, a)`.
fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8, u8)> {
    let h = hex.strip_prefix('#').unwrap_or(hex);
    let parse = |s: &str| u8::from_str_radix(s, 16).ok();
    match h.len() {
        3 => Some((
            parse(&h[0..1].repeat(2))?,
            parse(&h[1..2].repeat(2))?,
            parse(&h[2..3].repeat(2))?,
            255,
        )),
        6 => Some((parse(&h[0..2])?, parse(&h[2..4])?, parse(&h[4..6])?, 255)),
        8 => Some((
            parse(&h[0..2])?,
            parse(&h[2..4])?,
            parse(&h[4..6])?,
            parse(&h[6..8])?,
        )),
        _ => None,
    }
}

/// `LinearGradient.create(options)` — bound method on the native
/// `LinearGradient` const-object. `args = [ctx, options]` where
/// `options = { start: [x,y], end: [x,y], stops: [{offset, color}] }`.
/// Each stop's `color` is decoded via `extract_color` (downcasts
/// `ColorOpaque`), so it accepts Rust-owned `Color` handles directly — unlike
/// the low-level `createLinearGradient` which takes `{r,g,b,a}` structs.
pub(crate) fn tur_linear_gradient_create(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> boa_engine::JsResult<JsValue> {
    let opts = args.get_or_undefined(1).as_object().ok_or_else(|| {
        boa_engine::JsError::from(
            boa_engine::JsNativeError::typ()
                .with_message("LinearGradient.create: expected an options object"),
        )
    })?;
    let start = extract_offset_pair(&opts, "start", context).unwrap_or((0.0, 0.0));
    let end = extract_offset_pair(&opts, "end", context).unwrap_or((0.0, 0.0));

    let mut stops: Vec<GradientStop> = Vec::new();
    if let Ok(stops_val) = opts.get(js_string!("stops"), context) {
        if let Some(stops_obj) = stops_val.as_object() {
            if let Ok(arr) = boa_engine::object::builtins::JsArray::from_object(stops_obj.clone()) {
                let len = arr.length(context).unwrap_or(0);
                for i in 0..len {
                    let Ok(stop_val) = arr.at(i as i64, context) else {
                        continue;
                    };
                    let Some(stop_obj) = stop_val.as_object() else {
                        continue;
                    };
                    let offset = stop_obj
                        .get(js_string!("offset"), context)
                        .ok()
                        .and_then(|v| v.as_number())
                        .unwrap_or(0.0) as f32;
                    let color_val = stop_obj
                        .get(js_string!("color"), context)
                        .unwrap_or(JsValue::undefined());
                    let color = extract_color(&color_val, context).unwrap_or(Color::rgba(0, 0, 0, 0));
                    stops.push(GradientStop { offset, color });
                }
            }
        }
    }

    let brush = Brush::LinearGradient { start, end, stops };
    let opaque = BoaOpaque::new(BrushOpaque(brush), context);
    Ok(opaque.object().clone().into())
}

pub(crate) fn extract_color(value: &JsValue, context: &mut Context) -> Option<Color> {
    let obj = value.as_object()?;
    // Fast path: a Rust `ColorOpaque` (what `Color.hex/rgb/rgba` produce).
    if let Some(c) = obj.downcast_ref::<ColorOpaque>() {
        return Some(c.0);
    }
    // Legacy fallback: a plain `{ r, g, b, a }` object.
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

#[allow(dead_code)] fn extract_stops(
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

#[allow(dead_code)] pub(crate) fn extract_brush(value: &JsValue, context: &mut Context) -> Option<Brush> {
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
