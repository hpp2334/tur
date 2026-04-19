use crate::impl_dyn_element;
use boa_engine::{Context, JsString, JsValue};
use tur_element_tree::Element;
use tur_element_tree::ElementKind;
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

    fn name(&self) -> &'static str {
        "tur_text"
    }
}

impl_dyn_element!(TextElement);

impl crate::elements::BoaElement for TextElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        let key_str = key.to_std_string_escaped();
        match key_str.as_str() {
            "content" => {
                if let Some(s) = value.as_string() {
                    self.content = s.to_std_string_escaped();
                }
            }
            "fontSize" => {
                self.font_size = value.as_number().unwrap_or(14.0);
            }
            "color" => {
                if let Some(s) = value.as_string() {
                    self.color = Some(s.to_std_string_escaped());
                }
            }
            _ => {}
        }
    }
}
