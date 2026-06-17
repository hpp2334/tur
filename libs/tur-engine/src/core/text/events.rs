use boa_engine::{js_string, Context, JsValue};

use crate::core::edgy_event::EventArg;

// ---------------------------------------------------------------------------
// Text-editing event payloads — JS callback arguments emitted via
// TextEditingController (input, cursor, selection, composition).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct InputEvent {
    pub value: String,
    pub enter: bool,
}

#[derive(Clone)]
pub struct CursorChangeEvent {
    pub position: usize,
}

#[derive(Clone)]
pub struct SelectionChangeEvent {
    pub anchor: usize,
    pub end: usize,
}

#[derive(Clone)]
pub struct CompositionStartEvent;

#[derive(Clone)]
pub struct CompositionUpdateEvent {
    pub text: String,
}

#[derive(Clone)]
pub struct CompositionEndEvent {
    pub text: String,
}

impl EventArg for InputEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![
            JsValue::from(js_string!(self.value.as_str())),
            JsValue::from(self.enter),
        ]
    }
}

impl EventArg for CursorChangeEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![JsValue::from(self.position as f64)]
    }
}

impl EventArg for SelectionChangeEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![
            JsValue::from(self.anchor as f64),
            JsValue::from(self.end as f64),
        ]
    }
}

impl EventArg for CompositionStartEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        Vec::new()
    }
}

impl EventArg for CompositionUpdateEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![JsValue::from(js_string!(self.text.as_str()))]
    }
}

impl EventArg for CompositionEndEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![JsValue::from(js_string!(self.text.as_str()))]
    }
}
