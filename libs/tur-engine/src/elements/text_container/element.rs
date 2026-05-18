use boa_engine::{Context, JsString, JsValue};

use crate::core::elements::{ElementOnUpdate, ElementTrace};
use crate::elements::text::text_layout::TextLayoutData;

pub struct TextContainerElement {
    pub(crate) default_font_size: f64,
    pub(crate) cached_layout: Option<TextLayoutData>,
}

impl Default for TextContainerElement {
    fn default() -> Self {
        Self::new()
    }
}

impl TextContainerElement {
    pub fn new() -> Self {
        TextContainerElement {
            default_font_size: 14.0,
            cached_layout: None,
        }
    }
}

impl ElementTrace for TextContainerElement {}

impl ElementOnUpdate for TextContainerElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "fontSize" {
            self.default_font_size = value.as_number().unwrap_or(14.0);
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        if key.to_std_string_escaped().as_str() == "fontSize" {
            self.default_font_size = 14.0;
        }
    }
}
