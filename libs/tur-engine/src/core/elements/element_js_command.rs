use std::any::Any;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::js_command::AnyJsCommand;

pub trait ElementJsCallbackEmitter: 'static {
    fn emit_js_callback(
        &self,
        command: AnyJsCommand,
        context: &mut Context,
    ) -> Option<(JsObject, Vec<JsValue>)>;
}

pub(crate) fn dispatch_emit_js_callback<E: ElementJsCallbackEmitter>(
    any: &dyn Any,
    command: AnyJsCommand,
    context: &mut Context,
) -> Option<(JsObject, Vec<JsValue>)> {
    any.downcast_ref::<E>()
        .unwrap()
        .emit_js_callback(command, context)
}
