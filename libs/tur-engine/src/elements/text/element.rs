use boa_engine::{Context, JsString, JsValue};
use tur_shared::Color;

use crate::core::elements::ElementOnUpdate;
use crate::core::elements::ElementTrace;
use crate::elements::text::text_layout::TextLayoutData;

pub struct TextElement {
    pub(crate) content: String,
    pub(crate) font_size: f64,
    pub(crate) color: Option<Color>,
    pub(crate) cached_layout: Option<TextLayoutData>,
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
            cached_layout: None,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

impl ElementTrace for TextElement {
    fn trace_label(&self) -> String {
        format!("\"{}\"", self.content)
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
