use boa_engine::{Context, JsString, JsValue};

use crate::core::elements::{ElementOnGesture, ElementOnKeyboard, ElementOnUpdate};
use crate::core::elements::ElementTrace;

#[derive(Clone, Default)]
pub struct PointerInteractElement;

impl PointerInteractElement {
    pub fn new() -> Self {
        Self
    }
}

impl ElementTrace for PointerInteractElement {}
impl ElementOnUpdate for PointerInteractElement {
    fn set_prop(&mut self, _ctx: &mut Context, _key: &JsString, _value: &JsValue) {}
}

impl ElementOnKeyboard for PointerInteractElement {}
impl ElementOnGesture for PointerInteractElement {}
