use std::any::Any;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsValue};

use crate::core::js_command::AnyJsCommand;

pub trait ElementJsCallbackEmitter: 'static {
    fn emit_js_callback(
        &self,
        context: &mut Context,
        command: AnyJsCommand,
    ) -> Option<(JsFunction, Vec<JsValue>)>;
}

pub(crate) fn dispatch_emit_js_callback<E: ElementJsCallbackEmitter>(
    any: &dyn Any,
    context: &mut Context,
    command: AnyJsCommand,
) -> Option<(JsFunction, Vec<JsValue>)> {
    any.downcast_ref::<E>()
        .unwrap()
        .emit_js_callback(context, command)
}
