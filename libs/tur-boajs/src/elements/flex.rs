use crate::impl_dyn_element;
use boa_engine::{Context, JsString, JsValue};
use num_traits::FromPrimitive;
use tur_element_tree::{Element, ElementKind};
use tur_render_tree::{Axis, CrossAxisAlignment, FlexRenderObject, MainAxisAlignment};

#[derive(Clone)]
pub struct FlexElement {
    direction: Axis,
    main_alignment: MainAxisAlignment,
    cross_alignment: CrossAxisAlignment,
}

impl Default for FlexElement {
    fn default() -> Self {
        Self::new()
    }
}

impl FlexElement {
    pub fn new() -> Self {
        FlexElement {
            direction: Axis::Vertical,
            main_alignment: MainAxisAlignment::Start,
            cross_alignment: CrossAxisAlignment::Center,
        }
    }
}

impl Element for FlexElement {
    type TypedRenderObject = FlexRenderObject;

    fn to_render_object(&self) -> FlexRenderObject {
        FlexRenderObject::new(self.direction, self.main_alignment, self.cross_alignment)
    }

    fn kind(&self) -> ElementKind {
        ElementKind::new("tur_flex")
    }
}

impl_dyn_element!(FlexElement);

impl crate::elements::BoaElement for FlexElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "direction" {
            if let Some(n) = value.as_number() {
                self.direction = Axis::from_i32(n as i32).unwrap_or(self.direction);
            }
        } else if *key == "mainAlignment" {
            if let Some(n) = value.as_number() {
                self.main_alignment =
                    MainAxisAlignment::from_i32(n as i32).unwrap_or(self.main_alignment);
            }
        } else if *key == "crossAlignment" {
            if let Some(n) = value.as_number() {
                self.cross_alignment =
                    CrossAxisAlignment::from_i32(n as i32).unwrap_or(self.cross_alignment);
            }
        }
    }
}
