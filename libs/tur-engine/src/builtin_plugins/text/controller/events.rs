#![allow(dead_code)] // payload fields kept for editing-surface parity
use crate::core::edgy::mutation::MutationPayload;
use crate::core::edgy::value::Value;

// ---------------------------------------------------------------------------
// Text-editing event payloads — callback arguments emitted via
// TextEditingController (input, cursor, selection, composition). Plain
// markers cross with empty args (the default `to_value_args`); the input
// event carries its text + enter flag (the kit's typed `InputEvent`).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct InputEvent {
    pub(crate) value: String,
    pub(crate) enter: bool,
}

#[derive(Clone)]
pub struct CursorChangeEvent {
    pub(crate) position: usize,
}

#[derive(Clone)]
pub struct SelectionChangeEvent {
    pub(crate) anchor: usize,
    pub(crate) end: usize,
}

#[derive(Clone)]
pub struct CompositionStartEvent;

#[derive(Clone)]
pub struct CompositionUpdateEvent {
    pub(crate) text: String,
}

#[derive(Clone)]
pub struct CompositionEndEvent {
    pub(crate) text: String,
}

impl MutationPayload for InputEvent {
    /// Native crossing (the rut rail): `[value, enter]` — the kit's
    /// `InputEvent` event value decodes both fields.
    fn to_value_args(&self) -> Vec<Value> {
        vec![Value::str(self.value.as_str()), Value::Bool(self.enter)]
    }
}
impl MutationPayload for CursorChangeEvent {}
impl MutationPayload for SelectionChangeEvent {}
impl MutationPayload for CompositionStartEvent {}
impl MutationPayload for CompositionUpdateEvent {}
impl MutationPayload for CompositionEndEvent {}
