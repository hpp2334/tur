use crate::core::render::brush::{Brush, Color, GradientStop};
use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsArgs, JsValue};

use crate::core::js_runtime::BoaOpaque;
use crate::core::js_runtime::helpers::{ConstEntry, FnEntry, Ptr};
use crate::core::js_runtime::js_value::FromJs;
use crate::core::js_runtime::module_loader::bound_native;
use crate::core::render::brush::opaque::{BrushOpaque, ColorOpaque};
use std::str::FromStr;

/// Bridge function table entries for the color domain.
pub fn fns() -> Vec<FnEntry> {
    vec![
        ("createColor", 4, tur_create_color as Ptr),
        ("colorLerp", 3, tur_color_lerp as Ptr),
        ("createLinearGradient", 5, tur_create_linear_gradient as Ptr),
    ]
}

/// Constant exports for the color domain: the `Color` and `LinearGradient`
/// const-objects. Each is a namespace of bound native builders
/// (`Color.rgb/rgba/hex`, `LinearGradient.create`) that forward to the
/// ctx-first color bridge fns. Users never `new Color()`.
pub fn consts(context: &mut Context, ctx_val: JsValue) -> Vec<ConstEntry> {
    let color_obj = JsObject::with_object_proto(context.intrinsics());
    let _ = color_obj.create_data_property(
        js_string!("rgb"),
        JsValue::from(bound_native(
            context,
            ctx_val.clone(),
            tur_color_rgb,
            3,
            "rgb",
        )),
        context,
    );
    let _ = color_obj.create_data_property(
        js_string!("rgba"),
        JsValue::from(bound_native(
            context,
            ctx_val.clone(),
            tur_color_rgba,
            4,
            "rgba",
        )),
        context,
    );
    let _ = color_obj.create_data_property(
        js_string!("hex"),
        JsValue::from(bound_native(
            context,
            ctx_val.clone(),
            tur_color_hex,
            1,
            "hex",
        )),
        context,
    );

    let linear_obj = JsObject::with_object_proto(context.intrinsics());
    let _ = linear_obj.create_data_property(
        js_string!("create"),
        JsValue::from(bound_native(
            context,
            ctx_val,
            tur_linear_gradient_create,
            1,
            "create",
        )),
        context,
    );

    vec![
        ("Color", color_obj.into()),
        ("LinearGradient", linear_obj.into()),
    ]
}

// ---------------------------------------------------------------------------
// Bridge functions (opaque wrappers; Color/Brush FromJs impls live in
// tur-engine core).
// ---------------------------------------------------------------------------

/// Bridge function `createColor(r, g, b, a)` → JS opaque wrapping `Color`.
/// Called by the TS `Color` class so that Rust can read the value via
/// `downcast_ref::<ColorOpaque>()` without needing a boa `Context`.
fn tur_create_color(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> boa_engine::JsResult<JsValue> {
    // `args[0]` is the bound bridge ctx (prepended by `bound_native` when the
    // `tur:std` module is built); the real RGBA args follow at [1..].
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
fn tur_color_lerp(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> boa_engine::JsResult<JsValue> {
    // `args[0]` is the bound bridge ctx (see `tur_create_color`); real args at [1..].
    let a = Color::from_js(args.get_or_undefined(1)).map_err(|_| {
        boa_engine::JsNativeError::typ().with_message("colorLerp: `begin` must be a Color")
    })?;
    let b = Color::from_js(args.get_or_undefined(2)).map_err(|_| {
        boa_engine::JsNativeError::typ().with_message("colorLerp: `end` must be a Color")
    })?;
    let t = args.get_or_undefined(3).as_number().unwrap_or(0.0);
    let out = Color::lerp(a, b, t);
    let opaque = BoaOpaque::new(ColorOpaque(out), context);
    Ok(opaque.object().clone().into())
}

/// Bridge function `createLinearGradient(startX, startY, endX, endY, stops)`
/// where stops is an array of `{offset, r, g, b, a}`. Returns JS opaque
/// wrapping `Brush::LinearGradient`.
fn tur_create_linear_gradient(
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
    if let Some(stops_val) = args.get_or_undefined(5).as_object()
        && let Ok(arr) = boa_engine::object::builtins::JsArray::from_object(stops_val.clone())
    {
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
                let r = stop_obj
                    .get(js_string!("r"), context)
                    .ok()
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0) as u8;
                let g = stop_obj
                    .get(js_string!("g"), context)
                    .ok()
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0) as u8;
                let b = stop_obj
                    .get(js_string!("b"), context)
                    .ok()
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0) as u8;
                let a = stop_obj
                    .get(js_string!("a"), context)
                    .ok()
                    .and_then(|v| v.as_number())
                    .unwrap_or(255.0) as u8;
                stops.push(GradientStop {
                    offset,
                    color: Color::rgba(r, g, b, a),
                });
            }
        }
    }
    let brush = Brush::LinearGradient { start, end, stops };
    let opaque = BoaOpaque::new(BrushOpaque(brush), context);
    Ok(opaque.object().clone().into())
}

/// `Color.rgb(r, g, b)` — bound method on the native `Color` const-object.
/// `args = [ctx, r, g, b]`; forwards to `tur_create_color` with `a = 255`.
fn tur_color_rgb(
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
fn tur_color_rgba(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> boa_engine::JsResult<JsValue> {
    tur_create_color(_this, args, context)
}

/// `Color.hex("#RRGGBB[AA]" | "#RGB")` — bound method. Parses the hex string
/// via [`Color::from_str`] and wraps it as a `ColorOpaque`.
fn tur_color_hex(
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
    let color = Color::from_str(&hex).map_err(|_| {
        boa_engine::JsError::from(
            boa_engine::JsNativeError::typ()
                .with_message(format!("Color.hex: invalid hex color: {hex}")),
        )
    })?;
    let opaque = BoaOpaque::new(ColorOpaque(color), context);
    Ok(opaque.object().clone().into())
}

/// `LinearGradient.create(options)` — bound method on the native
/// `LinearGradient` const-object. `args = [ctx, options]` where
/// `options = { start: [x,y], end: [x,y], stops: [{offset, color}] }`.
/// Each stop's `color` is decoded via `Color::from_js` (downcasts
/// `ColorOpaque`), so it accepts Rust-owned `Color` handles directly — unlike
/// the low-level `createLinearGradient` which takes `{r,g,b,a}` structs.
fn tur_linear_gradient_create(
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
    let start = offset_pair(&opts, "start", context).unwrap_or((0.0, 0.0));
    let end = offset_pair(&opts, "end", context).unwrap_or((0.0, 0.0));

    let mut stops: Vec<GradientStop> = Vec::new();
    if let Ok(stops_val) = opts.get(js_string!("stops"), context)
        && let Some(stops_obj) = stops_val.as_object()
        && let Ok(arr) = boa_engine::object::builtins::JsArray::from_object(stops_obj.clone())
    {
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
            let color = Color::from_js(&color_val).unwrap_or(Color::rgba(0, 0, 0, 0));
            stops.push(GradientStop { offset, color });
        }
    }

    let brush = Brush::LinearGradient { start, end, stops };
    let opaque = BoaOpaque::new(BrushOpaque(brush), context);
    Ok(opaque.object().clone().into())
}

/// Read a `[x, y]` numeric array property. `None` if absent or malformed.
fn offset_pair(
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
