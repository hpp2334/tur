//! Animation controller bridge — `createAnimationController`.

use boa_engine::class::Class;
use boa_engine::{Context, JsResult, JsValue};

use crate::core::animation::AnimationController;
use crate::core::bridge::helpers::{extract_ctx, FnEntry, Ptr};

pub(crate) fn fns() -> Vec<FnEntry> {
    vec![("createAnimationController", 2, tur_create_animation_controller as Ptr)]
}

fn tur_create_animation_controller(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_ctx(args)?;
    let data = AnimationController::data_constructor(&JsValue::undefined(), &args[1..], context)?;
    let obj = AnimationController::from_data(data, context)?;
    if let Some(mut ctrl) = obj.downcast_mut::<AnimationController>() {
        ctrl.set_animation_manager(js_ctx.animation_manager.clone());
        ctrl.set_mutation_queue(js_ctx.mutation_queue.clone());
    }
    Ok(obj.upcast().clone().into())
}
