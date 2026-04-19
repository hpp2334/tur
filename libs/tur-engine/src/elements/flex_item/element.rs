use boa_engine::{Context, JsString, JsValue};

use crate::core::elements::ElementOnUpdate;

#[derive(Clone, Default)]
pub struct FlexItemElement;

impl FlexItemElement {
    pub fn new() -> Self {
        FlexItemElement
    }
}

impl ElementOnUpdate for FlexItemElement {
    fn set_prop(&mut self, _ctx: &mut Context, _key: &JsString, _value: &JsValue) {}
}
