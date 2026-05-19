use boa_engine::{Context, JsString, JsValue};
use num_traits::FromPrimitive;
use tur_shared::StackFit;

use crate::core::elements::ElementOnUpdate;
use crate::core::elements::ElementTrace;

#[derive(Clone)]
pub struct StackElement {
    pub(crate) fit: StackFit,
}

impl Default for StackElement {
    fn default() -> Self {
        Self::new()
    }
}

impl StackElement {
    pub fn new() -> Self {
        StackElement {
            fit: StackFit::Loose,
        }
    }
}

impl ElementTrace for StackElement {
    fn trace_label(&self) -> String {
        format!("fit={:?}", self.fit)
    }
}

impl ElementOnUpdate for StackElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "fit" {
            if let Some(n) = value.as_number() {
                self.fit = StackFit::from_i32(n as i32).unwrap_or(self.fit);
            }
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        if key.to_std_string_escaped().as_str() == "fit" {
            self.fit = StackFit::Loose;
        }
    }
}
