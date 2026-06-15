use boa_engine::class::Class;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::animation::AnimationController;
use crate::core::bridge::BoaOpaque;
use crate::core::bridge::TurJsContext;
use crate::core::element::ElementNodeId;
use crate::core::resource::ImageResource;
use crate::core::scroll::ScrollController;
use crate::core::text::TextEditingController;
use crate::elements::LazyListController;

#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurNodeHandle {
    pub id: ElementNodeId,
}

pub(crate) fn extract_ctx(args: &[JsValue]) -> JsResult<TurJsContext> {
    let obj = args.get_or_undefined(0).as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected TurJsContext as first argument"))
    })?;
    let ctx_ref = BoaOpaque::<TurJsContext>::wrap(&obj)
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message("expected TurJsContext as first argument")))?;
    Ok((*ctx_ref).clone())
}

pub(crate) fn tur_create_text_editing_controller(
    _this: &JsValue, args: &[JsValue], context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let data = TextEditingController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    Ok(TextEditingController::from_data(data, context)?.upcast().clone().into())
}

pub(crate) fn tur_create_scroll_controller(
    _this: &JsValue, args: &[JsValue], context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let data = ScrollController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    Ok(ScrollController::from_data(data, context)?.upcast().clone().into())
}

pub(crate) fn tur_create_lazy_list_controller(
    _this: &JsValue, args: &[JsValue], context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let data = LazyListController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    Ok(LazyListController::from_data(data, context)?.upcast().clone().into())
}

pub(crate) fn tur_create_animation_controller(
    _this: &JsValue, args: &[JsValue], context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let data = AnimationController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    let obj = AnimationController::from_data(data, context)?;
    if let Some(mut ctrl) = obj.downcast_mut::<AnimationController>() {
        ctrl.set_animation_manager(js_ctx.animation_manager.clone());
    }
    Ok(obj.upcast().clone().into())
}

pub(crate) fn tur_create_image_resource(
    _this: &JsValue, args: &[JsValue], context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let buffer_obj = args.get_or_undefined(1).as_object().ok_or_else(|| {
        JsError::from(JsNativeError::typ().with_message("expected ArrayBuffer or Uint8Array"))
    })?;
    let bytes = if let Ok(ta) = boa_engine::object::builtins::JsTypedArray::from_object(buffer_obj.clone()) {
        let offset = ta.byte_offset(context).unwrap_or(0);
        let len = ta.byte_length(context).unwrap_or(0);
        let buffer_val = ta.buffer(context)?;
        let buffer_obj = buffer_val.as_object().ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message("typed array has no backing buffer"))
        })?;
        let ab = boa_engine::object::builtins::JsArrayBuffer::from_object(buffer_obj)?;
        let full = ab.to_vec().unwrap_or_default();
        if offset + len > full.len() { full } else { full[offset..offset + len].to_vec() }
    } else if let Ok(ab) = boa_engine::object::builtins::JsArrayBuffer::from_object(buffer_obj.clone()) {
        ab.to_vec().unwrap_or_default()
    } else {
        return Err(JsError::from(
            JsNativeError::typ().with_message("expected ArrayBuffer or Uint8Array"),
        ));
    };
    let image = ImageResource::from_bytes(&bytes).ok_or_else(|| {
        JsError::from(JsNativeError::range().with_message("failed to decode image (supported: PNG, JPEG)"))
    })?;
    let id = js_ctx.resource_map.borrow_mut().insert_image(image);
    Ok(JsValue::from(id.as_u64() as f64))
}

pub(crate) fn tur_request_focus(
    _this: &JsValue, args: &[JsValue], _context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let obj = args.get_or_undefined(1).as_object().ok_or_else(|| {
        JsNativeError::typ().with_message("expected TurNodeHandle")
    })?;
    let handle = BoaOpaque::<TurNodeHandle>::wrap(&obj)
        .ok_or_else(|| JsNativeError::typ().with_message("expected TurNodeHandle"))?;
    let mut focus = js_ctx.focus_manager.borrow_mut();
    let mut js_eq = js_ctx.js_command_queue.borrow_mut();
    focus.set_focus(handle.id, &mut js_eq);
    Ok(JsValue::undefined())
}
