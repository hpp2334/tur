//! JS bridge for the `Image` element + image/svg resource factories.

use std::rc::Rc;

use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};

use crate::core::bridge::helpers::{extract_ctx, require_props_object, wrap_view, FnEntry, Ptr};
use crate::core::resource::ImageResource;

pub fn fns() -> Vec<FnEntry> {
    vec![
        ("Image", 2, tur_image as Ptr),
        ("createImageResource", 2, tur_create_image_resource as Ptr),
        ("createSvgResource", 2, tur_create_svg_resource as Ptr),
    ]
}

fn tur_image(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = super::ImageView::from_js(&props, context);
    Ok(wrap_view(Rc::new(spec), context))
}

fn tur_create_image_resource(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let buffer_obj = args.get_or_undefined(1).as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected ArrayBuffer or Uint8Array"))
    })?;
    let bytes = if let Ok(ta) =
        boa_engine::object::builtins::JsTypedArray::from_object(buffer_obj.clone())
    {
        let offset = ta.byte_offset(context).unwrap_or(0);
        let len = ta.byte_length(context).unwrap_or(0);
        let buffer_val = ta.buffer(context)?;
        let buffer_obj = buffer_val.as_object().ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message("typed array has no backing buffer"))
        })?;
        let ab = boa_engine::object::builtins::JsArrayBuffer::from_object(buffer_obj)?;
        let full = ab.to_vec().unwrap_or_default();
        if offset + len > full.len() {
            full
        } else {
            full[offset..offset + len].to_vec()
        }
    } else if let Ok(ab) =
        boa_engine::object::builtins::JsArrayBuffer::from_object(buffer_obj.clone())
    {
        ab.to_vec().unwrap_or_default()
    } else {
        return Err(JsError::from(
            JsNativeError::typ().with_message("expected ArrayBuffer or Uint8Array"),
        ));
    };
    let image = ImageResource::from_bytes(&bytes).ok_or_else(|| {
        JsError::from(
            JsNativeError::range()
                .with_message("failed to decode image (supported: PNG, JPEG)"),
        )
    })?;
    let id = js_ctx.resource_map.borrow_mut().insert_image(image);
    Ok(JsValue::from(id.as_u64() as f64))
}

fn tur_create_svg_resource(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let svg = args
        .get_or_undefined(1)
        .as_string()
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message("expected SVG string")))?;
    let svg_str = svg.to_std_string_escaped();
    let image = ImageResource::from_svg_str(&svg_str).ok_or_else(|| {
        JsError::from(JsNativeError::range().with_message("failed to parse/render SVG"))
    })?;
    let id = js_ctx.resource_map.borrow_mut().insert_image(image);
    Ok(JsValue::from(id.as_u64() as f64))
}
