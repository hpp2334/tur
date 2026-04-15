use boa_engine::class::Class;
use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::FunctionObjectBuilder;
use boa_engine::property::Attribute;
use boa_engine::Context;
use boa_engine::JsObject;
use tracing;

use crate::widget_bridge::TurAppContext;

pub fn init_bridge(context: &mut Context) -> JsObject {
    context
        .register_global_class::<TurAppContext>()
        .expect("failed to register TurAppContext class");

    let ctx = TurAppContext::default();
    let ctx_obj =
        TurAppContext::from_data(ctx, context).expect("failed to create TurAppContext instance");

    let realm = context.realm();
    let captured = ctx_obj.clone();
    let factory = FunctionObjectBuilder::new(realm, unsafe {
        NativeFunction::from_closure(move |_this, _args, _context| Ok(captured.clone().into()))
    })
    .length(0)
    .build();

    context
        .register_global_property(js_string!("createTurApp"), factory, Attribute::all())
        .expect("failed to register globalThis.createTurApp");

    tracing::info!("tur bridge initialized");

    ctx_obj
}
