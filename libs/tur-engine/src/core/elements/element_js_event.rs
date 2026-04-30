use std::any::Any;
use std::rc::Rc;

use boa_engine::Context;

pub trait ElementJsEventEmitter: 'static {
    fn flush_js_event(&mut self, event: Rc<dyn Any>, context: &mut Context);
}

pub(crate) fn dispatch_flush_js_event<E: ElementJsEventEmitter>(
    any: &mut dyn Any,
    event: Rc<dyn Any>,
    context: &mut Context,
) {
    any.downcast_mut::<E>().unwrap().flush_js_event(event, context);
}
