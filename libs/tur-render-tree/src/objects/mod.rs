mod container;
mod flex;
mod flex_item;
mod positioned;
mod stack;
mod text;

use std::collections::HashMap;

use tur_element::PropValue;
use tur_shared::ElementKind;

pub use container::ContainerRenderObject;
pub use flex::FlexRenderObject;
pub use flex_item::FlexItemRenderObject;
pub use positioned::PositionedRenderObject;
pub use stack::StackRenderObject;
pub use text::TextRenderObject;

use crate::render_object::RenderObject;

pub fn create_render_object(
    kind: ElementKind,
    props: &HashMap<String, PropValue>,
) -> Box<dyn RenderObject> {
    match kind {
        ElementKind::Flex => Box::new(FlexRenderObject::from_props(props)),
        ElementKind::FlexItem => Box::new(FlexItemRenderObject),
        ElementKind::Stack => Box::new(StackRenderObject::from_props(props)),
        ElementKind::Positioned => Box::new(PositionedRenderObject::from_props(props)),
        ElementKind::Container => Box::new(ContainerRenderObject::from_props(props)),
        ElementKind::Text => Box::new(TextRenderObject::from_props(props)),
    }
}

fn prop_str<'a>(props: &'a HashMap<String, PropValue>, key: &str) -> Option<&'a str> {
    props.get(key).and_then(|v| v.as_str())
}

fn prop_f64(props: &HashMap<String, PropValue>, key: &str) -> Option<f64> {
    props.get(key).and_then(|v| v.as_f64())
}
