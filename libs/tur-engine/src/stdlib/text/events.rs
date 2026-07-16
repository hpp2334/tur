use boa_engine::{js_string, Context, JsValue};

use crate::core::edgy_event::IntoJsArgs;

// ---------------------------------------------------------------------------
// Text-editing event payloads — JS callback arguments emitted via
// TextEditingController (input, cursor, selection, composition).
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

impl IntoJsArgs for InputEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![
            JsValue::from(js_string!(self.value.as_str())),
            JsValue::from(self.enter),
        ]
    }
}

impl IntoJsArgs for CursorChangeEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![JsValue::from(self.position as f64)]
    }
}

impl IntoJsArgs for SelectionChangeEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![
            JsValue::from(self.anchor as f64),
            JsValue::from(self.end as f64),
        ]
    }
}

impl IntoJsArgs for CompositionStartEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        Vec::new()
    }
}

impl IntoJsArgs for CompositionUpdateEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![JsValue::from(js_string!(self.text.as_str()))]
    }
}

impl IntoJsArgs for CompositionEndEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![JsValue::from(js_string!(self.text.as_str()))]
    }
}
