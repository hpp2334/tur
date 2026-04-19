use crate::impl_dyn_element;
use boa_engine::{Context, JsString, JsValue};
use tur_element_tree::Element;
use tur_element_tree::ElementKind;
use tur_render_tree::PositionedRenderObject;

#[derive(Clone, Default)]
pub struct PositionedElement {
    left: Option<f64>,
    top: Option<f64>,
    right: Option<f64>,
    bottom: Option<f64>,
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

impl Element for PositionedElement {
    type TypedRenderObject = PositionedRenderObject;

    fn to_render_object(&self) -> PositionedRenderObject {
        PositionedRenderObject::new(self.left, self.top, self.right, self.bottom)
    }

    fn kind(&self) -> ElementKind {
        ElementKind::new("tur_positioned")
    }

    fn name(&self) -> &'static str {
        "tur_positioned"
    }
}

impl_dyn_element!(PositionedElement);

impl crate::elements::BoaElement for PositionedElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        let key_str = key.to_std_string_escaped();
        let val = value.as_number().or_else(|| {
            value
                .as_string()
                .and_then(|s| s.to_std_string_escaped().parse::<f64>().ok())
        });
        match key_str.as_str() {
            "left" => self.left = val,
            "top" => self.top = val,
            "right" => self.right = val,
            "bottom" => self.bottom = val,
            _ => {}
        }
    }
}
