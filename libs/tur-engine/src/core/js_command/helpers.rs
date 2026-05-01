use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::keyboard::Modifiers;

pub fn build_key_event_object(
    key: &str,
    code: &str,
    modifiers: &Modifiers,
    context: &mut Context,
) -> JsValue {
    let proto = context.intrinsics().constructors().object().prototype();
    let obj = JsObject::from_proto_and_data(proto, ());

    let desc = boa_engine::property::PropertyDescriptor::builder()
        .writable(true)
        .enumerable(true)
        .configurable(true);

    obj.insert_property(
        boa_engine::js_string!("key"),
        desc.clone()
            .value(boa_engine::JsValue::from(boa_engine::js_string!(key)))
            .build(),
    );
    obj.insert_property(
        boa_engine::js_string!("code"),
        desc.clone()
            .value(boa_engine::JsValue::from(boa_engine::js_string!(code)))
            .build(),
    );
    obj.insert_property(
        boa_engine::js_string!("ctrl"),
        desc.clone()
            .value(boa_engine::JsValue::from(modifiers.ctrl))
            .build(),
    );
    obj.insert_property(
        boa_engine::js_string!("shift"),
        desc.clone()
            .value(boa_engine::JsValue::from(modifiers.shift))
            .build(),
    );
    obj.insert_property(
        boa_engine::js_string!("alt"),
        desc.clone()
            .value(boa_engine::JsValue::from(modifiers.alt))
            .build(),
    );
    obj.insert_property(
        boa_engine::js_string!("meta"),
        desc.value(boa_engine::JsValue::from(modifiers.meta))
            .build(),
    );

    obj.into()
}
