use crate::impl_dyn_element;
use boa_engine::{Context, JsString, JsValue};
use tur_element_tree::{Element, ElementKind};
use tur_render_tree::ContainerRenderObject;

#[derive(Clone, Default)]
pub struct ContainerElement {
    width: Option<f64>,
    height: Option<f64>,
    padding: Option<f64>,
    color: Option<String>,
}

impl ContainerElement {
    pub fn new() -> Self {
        ContainerElement {
            width: None,
            height: None,
            padding: None,
            color: None,
        }
    }
}

impl Element for ContainerElement {
    type TypedRenderObject = ContainerRenderObject;

    fn to_render_object(&self) -> ContainerRenderObject {
        ContainerRenderObject::new(self.width, self.height, self.padding, self.color.clone())
    }

    fn kind(&self) -> ElementKind {
        ElementKind::new("tur_container")
    }
}

impl_dyn_element!(ContainerElement);

impl crate::elements::BoaElement for ContainerElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "width" {
            self.width = value.as_number();
        } else if *key == "height" {
            self.height = value.as_number();
        } else if *key == "padding" {
            self.padding = value.as_number();
        } else if *key == "color" {
            if let Some(s) = value.as_string() {
                self.color = Some(s.to_std_string_escaped());
            }
        }
    }
}
