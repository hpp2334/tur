use crate::impl_dyn_element;
use boa_engine::{Context, JsString, JsValue};
use tur_element_tree::{Element, ElementKind};
use tur_render_tree::FlexItemRenderObject;

#[derive(Clone, Default)]
pub struct FlexItemElement;

impl FlexItemElement {
    pub fn new() -> Self {
        FlexItemElement
    }
}

impl Element for FlexItemElement {
    type TypedRenderObject = FlexItemRenderObject;

    fn to_render_object(&self) -> FlexItemRenderObject {
        FlexItemRenderObject
    }

    fn kind(&self) -> ElementKind {
        ElementKind::new("tur_flex_item")
    }
}

impl_dyn_element!(FlexItemElement);

impl crate::elements::BoaElement for FlexItemElement {
    fn set_prop(&mut self, _ctx: &mut Context, _key: &JsString, _value: &JsValue) {}
}
