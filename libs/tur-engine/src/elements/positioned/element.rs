use boa_engine::{Context, JsString, JsValue};
use tur_shared::AnimatableValue;

use crate::core::elements::ElementOnUpdate;
use crate::core::elements::ElementTrace;

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

impl ElementTrace for PositionedElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(v) = self.left {
            parts.push(format!("left={v}"));
        }
        if let Some(v) = self.top {
            parts.push(format!("top={v}"));
        }
        if let Some(v) = self.right {
            parts.push(format!("right={v}"));
        }
        if let Some(v) = self.bottom {
            parts.push(format!("bottom={v}"));
        }
        parts.join(" ")
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

    fn reset_prop(&mut self, key: &JsString) {
        match key.to_std_string_escaped().as_str() {
            "left" => self.left = None,
            "top" => self.top = None,
            "right" => self.right = None,
            "bottom" => self.bottom = None,
            _ => {}
        }
    }

    fn apply_animated(&mut self, key: &str, value: AnimatableValue) {
        let AnimatableValue::Float(v) = value else { return };
        match key {
            "left" => self.left = Some(v),
            "top" => self.top = Some(v),
            "right" => self.right = Some(v),
            "bottom" => self.bottom = Some(v),
            _ => {}
        }
    }

    fn get_animatable(&self, key: &str) -> Option<AnimatableValue> {
        match key {
            "left" => self.left.map(AnimatableValue::Float),
            "top" => self.top.map(AnimatableValue::Float),
            "right" => self.right.map(AnimatableValue::Float),
            "bottom" => self.bottom.map(AnimatableValue::Float),
            _ => None,
        }
    }
}
