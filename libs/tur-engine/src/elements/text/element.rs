use boa_engine::{Context, JsString, JsValue};
use tur_shared::Color;

use crate::core::elements::ElementOnUpdate;

#[derive(Clone)]
pub struct TextElement {
    pub(crate) content: String,
    pub(crate) font_size: f64,
    pub(crate) color: Option<Color>,
}

impl Default for TextElement {
    fn default() -> Self {
        Self::new()
    }
}

impl TextElement {
    pub fn new() -> Self {
        TextElement {
            content: String::new(),
            font_size: 14.0,
            color: None,
        }
    }
}

impl ElementOnUpdate for TextElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "content" {
            if let Some(s) = value.as_string() {
                self.content = s.to_std_string_escaped();
            }
        } else if *key == "fontSize" {
            self.font_size = value.as_number().unwrap_or(14.0);
        } else if *key == "color" {
            if let Some(s) = value.as_string() {
                self.color = s.to_std_string_escaped().parse().ok();
            }
        }
    }
}
