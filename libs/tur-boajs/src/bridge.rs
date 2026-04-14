use boa_engine::js_string;
use boa_engine::object::ObjectInitializer;
use boa_engine::property::Attribute;
use boa_engine::Context;
use tracing;

use crate::widget_bridge;

pub fn init_bridge(context: &mut Context) {
    let widget = widget_bridge::create_widget_namespace(context);

    let tur = ObjectInitializer::new(context)
        .property(js_string!("widget"), widget, Attribute::all())
        .build();

    context
        .register_global_property(js_string!("tur"), tur, Attribute::all())
        .expect("failed to register globalThis.tur");

    tracing::info!("tur bridge initialized");
}
