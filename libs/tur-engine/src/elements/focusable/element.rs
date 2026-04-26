use boa_engine::{Context, JsString, JsValue};

use crate::core::elements::ElementOnUpdate;
use crate::core::elements::ElementTrace;

#[derive(Clone, Default)]
pub struct FocusableElement;

impl FocusableElement {
    pub fn new() -> Self {
        Self
    }
}

impl ElementTrace for FocusableElement {}
impl ElementOnUpdate for FocusableElement {
    fn set_prop(&mut self, _ctx: &mut Context, _key: &JsString, _value: &JsValue) {}
}
