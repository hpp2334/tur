use boa_engine::{Context, JsString, JsValue};

use crate::core::elements::{ElementOnUpdate, ElementTrace};
use crate::elements::text::span_data::SpanData;
use crate::elements::text::text_layout::TextLayoutData;

pub struct TextContainerElement {
    pub(crate) default_font_size: f64,
    pub(crate) spans: Vec<SpanData>,
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
            spans: Vec::new(),
            cached_layout: None,
        }
    }

    pub fn spans(&self) -> &[SpanData] {
        &self.spans
    }
}

impl ElementTrace for TextContainerElement {}

impl ElementOnUpdate for TextContainerElement {
    fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue) {
        match key.to_std_string_escaped().as_str() {
            "fontSize" => {
                self.default_font_size = value.as_number().unwrap_or(14.0);
            }
            "spans" => {
                self.spans = crate::elements::text::span_data::extract_spans_from_js(value, ctx);
            }
            _ => {}
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        match key.to_std_string_escaped().as_str() {
            "fontSize" => self.default_font_size = 14.0,
            "spans" => self.spans.clear(),
            _ => {}
        }
    }
}
