use crate::impl_dyn_element;
use boa_engine::{Context, JsString, JsValue};
use tur_element_tree::Element;
use tur_render_tree::StackRenderObject;
use tur_trait::{ElementKind, StackFit};

#[derive(Clone)]
pub struct StackElement {
    fit: StackFit,
}

impl Default for StackElement {
    fn default() -> Self {
        Self::new()
    }
}

impl StackElement {
    pub fn new() -> Self {
        StackElement {
            fit: StackFit::Loose,
        }
    }
}

impl Element for StackElement {
    type TypedRenderObject = StackRenderObject;

    fn to_render_object(&self) -> StackRenderObject {
        StackRenderObject::new(self.fit)
    }

    fn kind(&self) -> ElementKind {
        ElementKind::new("tur_stack")
    }

    fn name(&self) -> &'static str {
        "tur_stack"
    }
}

impl_dyn_element!(StackElement);

impl crate::elements::BoaElement for StackElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        let key_str = key.to_std_string_escaped();
        if key_str == "fit" {
            if let Some(s) = value.as_string() {
                self.fit = match s.to_std_string_escaped().as_str() {
                    "loose" => StackFit::Loose,
                    "expand" => StackFit::Expand,
                    "passthrough" => StackFit::Passthrough,
                    _ => return,
                };
            } else if let Some(n) = value.as_number() {
                self.fit = match n as i32 {
                    0 => StackFit::Loose,
                    1 => StackFit::Expand,
                    2 => StackFit::Passthrough,
                    _ => return,
                };
            }
        }
    }
}
