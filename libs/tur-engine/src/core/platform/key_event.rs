//! Keyboard event payloads — engine contract types shared by the platform
//! layer (`ShellEvent::Key` wraps [`KeyEvent`]) and the input subsystems
//! that route key events to the focused element.
//!
//! JS-callback argument payloads ([`KeydownEvent`] / [`KeyupEvent`]) for
//! `onKeyDown$` / `onKeyUp$` mutations live here too — they reference
//! [`Modifiers`] which is also defined here.

use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsValue, js_string};

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


impl crate::core::edgy::mutation::MutationPayload for KeydownEvent {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue> {
        IntoJsArgs::to_js_args(self, ctx)
    }

    /// Native crossing (the rut rail): `[key, code, mods, kind]` —
    /// `mods` bit0 shift / bit1 ctrl / bit2 alt / bit3 meta, kind 0 = down.
    fn to_value_args(&self) -> Vec<crate::core::edgy::Value> {
        vec![
            crate::core::edgy::Value::str(self.key.as_str()),
            crate::core::edgy::Value::str(self.code.as_str()),
            crate::core::edgy::Value::Num(modifiers_bits(&self.modifiers) as f64),
            crate::core::edgy::Value::Num(0.0),
        ]
    }
}

impl crate::core::edgy::mutation::MutationPayload for KeyupEvent {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue> {
        IntoJsArgs::to_js_args(self, ctx)
    }

    /// Native crossing (the rut rail): `[key, code, mods, kind]`, kind 1 = up.
    fn to_value_args(&self) -> Vec<crate::core::edgy::Value> {
        vec![
            crate::core::edgy::Value::str(self.key.as_str()),
            crate::core::edgy::Value::str(self.code.as_str()),
            crate::core::edgy::Value::Num(modifiers_bits(&self.modifiers) as f64),
            crate::core::edgy::Value::Num(1.0),
        ]
    }
}

/// The modifier bit-pack shared by the native key crossings.
fn modifiers_bits(m: &Modifiers) -> u64 {
    (m.shift as u64) | ((m.ctrl as u64) << 1) | ((m.alt as u64) << 2) | ((m.meta as u64) << 3)
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
    let _ =
        obj.create_data_property_or_throw(js_string!("ctrl"), JsValue::from(modifiers.ctrl), ctx);
    let _ =
        obj.create_data_property_or_throw(js_string!("shift"), JsValue::from(modifiers.shift), ctx);
    let _ = obj.create_data_property_or_throw(js_string!("alt"), JsValue::from(modifiers.alt), ctx);
    let _ =
        obj.create_data_property_or_throw(js_string!("meta"), JsValue::from(modifiers.meta), ctx);
    let _ = Attribute::all();
    vec![obj.into()]
}
