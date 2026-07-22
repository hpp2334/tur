//! Keyboard event payloads — engine contract types shared by the platform
//! layer (`PlatformEvent::Key` wraps [`KeyEvent`]) and the input subsystems
//! that route key events to the focused element.
//!
//! JS-callback argument payloads ([`KeydownEvent`] / [`KeyupEvent`]) for
//! `onKeyDown$` / `onKeyUp$` mutations live here too — they reference
//! [`Modifiers`] which is also defined here.

use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::{js_string, Context, JsValue};

use crate::core::edgy::mutation::IntoJsArgs;

#[derive(Clone, Copy, Debug, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyEventType {
    Down,
    Up,
}

#[derive(Clone, Debug)]
pub struct KeyEvent {
    pub key: String,
    pub code: String,
    pub modifiers: Modifiers,
    pub event_type: KeyEventType,
}

// ---------------------------------------------------------------------------
// Keyboard event payloads — JS callback arguments for keydown / keyup.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct KeydownEvent {
    pub key: String,
    pub code: String,
    pub modifiers: Modifiers,
}

#[derive(Clone)]
pub struct KeyupEvent {
    pub key: String,
    pub code: String,
    pub modifiers: Modifiers,
}

impl IntoJsArgs for KeydownEvent {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue> {
        build_key_event_object(&self.key, &self.code, &self.modifiers, ctx)
    }
}

impl IntoJsArgs for KeyupEvent {
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
