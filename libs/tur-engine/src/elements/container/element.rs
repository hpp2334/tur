use boa_engine::{Context, JsString, JsValue};
use tur_shared::Color;

use crate::core::elements::{ElementOnKeyboard, ElementOnUpdate};
use crate::core::elements::ElementTrace;

#[derive(Clone, Default)]
pub struct ContainerElement {
    pub(crate) width: Option<f64>,
    pub(crate) height: Option<f64>,
    pub(crate) padding: Option<f64>,
    pub(crate) color: Option<Color>,
}

impl ContainerElement {
    pub fn new() -> Self {
        ContainerElement {
            width: None,
            height: None,
            padding: None,
            color: None,
        }
    }
}

impl ElementTrace for ContainerElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(w) = self.width {
            parts.push(format!("width={w}"));
        }
        if let Some(h) = self.height {
            parts.push(format!("height={h}"));
        }
        if let Some(p) = self.padding {
            parts.push(format!("padding={p}"));
        }
        if let Some(c) = self.color {
            parts.push(format!("color={c}"));
        }
        parts.join(" ")
    }
}

impl ElementOnUpdate for ContainerElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "width" {
            self.width = value.as_number();
        } else if *key == "height" {
            self.height = value.as_number();
        } else if *key == "padding" {
            self.padding = value.as_number();
        } else if *key == "color" {
            if let Some(s) = value.as_string() {
                self.color = s.to_std_string_escaped().parse().ok();
            }
        }
    }
}

impl ElementOnKeyboard for ContainerElement {}
