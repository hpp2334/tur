use crate::impl_dyn_element;
use boa_engine::{Context, JsString, JsValue};
use num_traits::FromPrimitive;
use tur_element_tree::{Element, ElementKind};
use tur_render_tree::{StackFit, StackRenderObject};

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
}

impl_dyn_element!(StackElement);

impl crate::elements::BoaElement for StackElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "fit" {
            if let Some(n) = value.as_number() {
                self.fit = StackFit::from_i32(n as i32).unwrap_or(self.fit);
            }
        }
    }
}
