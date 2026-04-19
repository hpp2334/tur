use boa_engine::{Context, JsString, JsValue};

use crate::core::elements::ElementOnUpdate;

#[derive(Clone, Default)]
pub struct PositionedElement {
    pub(crate) left: Option<f64>,
    pub(crate) top: Option<f64>,
    pub(crate) right: Option<f64>,
    pub(crate) bottom: Option<f64>,
}

impl PositionedElement {
    pub fn new() -> Self {
        PositionedElement {
            left: None,
            top: None,
            right: None,
            bottom: None,
        }
    }
}

impl ElementOnUpdate for PositionedElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        let val = value.as_number().or_else(|| {
            value
                .as_string()
                .and_then(|s| s.to_std_string_escaped().parse::<f64>().ok())
        });
        if *key == "left" {
            self.left = val;
        } else if *key == "top" {
            self.top = val;
        } else if *key == "right" {
            self.right = val;
        } else if *key == "bottom" {
            self.bottom = val;
        }
    }
}
