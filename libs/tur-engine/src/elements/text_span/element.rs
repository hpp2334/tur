use std::fmt;

use boa_engine::{Context, JsString, JsValue};
use tur_shared::{Color, ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::elements::{ElementOnUpdate, ElementTrace};
use crate::core::layout::LayoutContext;
use crate::core::render::{Canvas, ElementRender, PaintContext};

pub struct TextSpanElement {
    pub(crate) content: String,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) underline: bool,
    pub(crate) font_size: Option<f64>,
    pub(crate) color: Option<Color>,
}

impl Default for TextSpanElement {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TextSpanElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextSpanElement")
            .field("content", &self.content)
            .field("bold", &self.bold)
            .field("italic", &self.italic)
            .field("underline", &self.underline)
            .finish()
    }
}

impl TextSpanElement {
    pub fn new() -> Self {
        TextSpanElement {
            content: String::new(),
            bold: false,
            italic: false,
            underline: false,
            font_size: None,
            color: None,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

impl ElementTrace for TextSpanElement {
    fn trace_label(&self) -> String {
        if self.content.is_empty() {
            String::new()
        } else {
            format!("\"{}\"", self.content)
        }
    }
}

impl ElementOnUpdate for TextSpanElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "content" {
            if let Some(s) = value.as_string() {
                self.content = s.to_std_string_escaped();
            }
        } else if *key == "bold" {
            self.bold = value
                .as_boolean()
                .unwrap_or(value.to_boolean());
        } else if *key == "italic" {
            self.italic = value
                .as_boolean()
                .unwrap_or(value.to_boolean());
        } else if *key == "underline" {
            self.underline = value
                .as_boolean()
                .unwrap_or(value.to_boolean());
        } else if *key == "fontSize" {
            self.font_size = value.as_number().and_then(|v| if v == 0.0 { None } else { Some(v) });
        } else if *key == "color" {
            if let Some(s) = value.as_string() {
                self.color = s.to_std_string_escaped().parse().ok();
            } else if value.is_null() || value.is_undefined() {
                self.color = None;
            }
        }
    }
}

impl crate::core::layout::ElementLayout for TextSpanElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        _cx: &mut LayoutContext,
    ) -> Size {
        constraints.constrain(Size::ZERO)
    }

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], _cx: &mut LayoutContext) {}
}

impl ElementRender for TextSpanElement {
    fn type_name(&self) -> &'static str {
        "tur_text_span"
    }

    fn paint(
        &self,
        _canvas: &mut dyn Canvas,
        _offset: Offset,
        _layout: &ComputedLayout,
        _children: &[ElementNodeId],
        _paint_ctx: &PaintContext,
    ) {
    }
}
