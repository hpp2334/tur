use boa_engine::{Context, JsString, JsValue};

use crate::core::elements::ElementOnUpdate;

#[derive(Clone, Default)]
pub struct PointerInteractElement;

impl PointerInteractElement {
    pub fn new() -> Self {
        Self
    }
}

impl ElementOnUpdate for PointerInteractElement {
    fn set_prop(&mut self, _ctx: &mut Context, _key: &JsString, _value: &JsValue) {}
}
