use std::any::Any;

use boa_engine::Context;

use crate::core::js_event::AnyJsEvent;

pub trait ElementJsEventEmitter: 'static {
    fn flush_js_event(&mut self, event: AnyJsEvent, context: &mut Context);
}

pub(crate) fn dispatch_flush_js_event<E: ElementJsEventEmitter>(
    any: &mut dyn Any,
    event: AnyJsEvent,
    context: &mut Context,
) {
    any.downcast_mut::<E>().unwrap().flush_js_event(event, context);
}
