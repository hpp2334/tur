use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::{js_string, Context, JsValue};

use tur_engine::core::edgy_event::EventArg;
use tur_engine::core::keyboard::Modifiers;

// ---------------------------------------------------------------------------
// Keyboard event payloads — JS callback arguments for keydown / keyup.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct KeydownEvent {
    pub(crate) key: String,
    pub(crate) code: String,
    pub(crate) modifiers: Modifiers,
}

#[derive(Clone)]
pub struct KeyupEvent {
    pub(crate) key: String,
    pub(crate) code: String,
    pub(crate) modifiers: Modifiers,
}

impl EventArg for KeydownEvent {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue> {
        build_key_event_object(&self.key, &self.code, &self.modifiers, ctx)
    }
}

impl EventArg for KeyupEvent {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue> {
        build_key_event_object(&self.key, &self.code, &self.modifiers, ctx)
    }
}

fn build_key_event_object(
    key: &str,
    code: &str,
    modifiers: &Modifiers,
    ctx: &mut Context,
) -> Vec<JsValue> {
    let proto = ctx.intrinsics().constructors().object().prototype();
    let obj = JsObject::from_proto_and_data(proto, ());
    let _ = obj.create_data_property_or_throw(js_string!("key"), js_string!(key), ctx);
    let _ = obj.create_data_property_or_throw(js_string!("code"), js_string!(code), ctx);
    let _ = obj.create_data_property_or_throw(js_string!("ctrl"), JsValue::from(modifiers.ctrl), ctx);
    let _ = obj.create_data_property_or_throw(js_string!("shift"), JsValue::from(modifiers.shift), ctx);
    let _ = obj.create_data_property_or_throw(js_string!("alt"), JsValue::from(modifiers.alt), ctx);
    let _ = obj.create_data_property_or_throw(js_string!("meta"), JsValue::from(modifiers.meta), ctx);
    let _ = Attribute::all();
    vec![obj.into()]
}
