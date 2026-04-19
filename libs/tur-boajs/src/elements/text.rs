use crate::impl_dyn_element;
use boa_engine::{Context, JsString, JsValue};
use tur_element_tree::{Element, ElementKind};
use tur_render_tree::TextRenderObject;

#[derive(Clone)]
pub struct TextElement {
    content: String,
    font_size: f64,
    color: Option<String>,
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

impl Element for TextElement {
    type TypedRenderObject = TextRenderObject;

    fn to_render_object(&self) -> TextRenderObject {
        TextRenderObject::new(self.content.clone(), self.font_size, self.color.clone())
    }

    fn kind(&self) -> ElementKind {
        ElementKind::new("tur_text")
    }
}

impl_dyn_element!(TextElement);

impl crate::elements::BoaElement for TextElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "content" {
            if let Some(s) = value.as_string() {
                self.content = s.to_std_string_escaped();
            }
        } else if *key == "fontSize" {
            self.font_size = value.as_number().unwrap_or(14.0);
        } else if *key == "color" {
            if let Some(s) = value.as_string() {
                self.color = Some(s.to_std_string_escaped());
            }
        }
    }
}
