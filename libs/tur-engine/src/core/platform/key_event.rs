//! Keyboard event payloads — engine contract types shared by the platform
//! layer (`ShellEvent::Key` wraps [`KeyEvent`]) and the input subsystems
//! that route key events to the focused element.
//!
//! JS-callback argument payloads ([`KeydownEvent`] / [`KeyupEvent`]) for
//! `onKeyDown$` / `onKeyUp$` mutations live here too — they reference
//! [`Modifiers`] which is also defined here.

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
