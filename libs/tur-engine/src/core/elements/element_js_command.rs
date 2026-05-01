use std::any::Any;

use boa_engine::Context;

use crate::core::js_command::AnyJsCommand;

pub trait ElementJsCommandEmitter: 'static {
    fn flush_js_command(&mut self, command: AnyJsCommand, context: &mut Context);
}

pub(crate) fn dispatch_flush_js_command<E: ElementJsCommandEmitter>(
    any: &mut dyn Any,
    command: AnyJsCommand,
    context: &mut Context,
) {
    any.downcast_mut::<E>().unwrap().flush_js_command(command, context);
}
