#![allow(dead_code)] // payload fields kept for editing-surface parity
use crate::core::edgy::mutation::MutationPayload;

// ---------------------------------------------------------------------------
// Text-editing event payloads — callback arguments emitted via
// TextEditingController (input, cursor, selection, composition). All plain
// markers: the callbacks read their payload from closure captures, so every
// payload crosses with empty args (the default `to_value_args`).
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

impl MutationPayload for InputEvent {}
impl MutationPayload for CursorChangeEvent {}
impl MutationPayload for SelectionChangeEvent {}
impl MutationPayload for CompositionStartEvent {}
impl MutationPayload for CompositionUpdateEvent {}
impl MutationPayload for CompositionEndEvent {}
