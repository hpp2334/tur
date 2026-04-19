mod container;
mod flex;
mod flex_item;
mod positioned;
mod stack;
mod text;

pub use container::ContainerElement;
pub use flex::FlexElement;
pub use flex_item::FlexItemElement;
pub use positioned::PositionedElement;
pub use stack::StackElement;
pub use text::TextElement;

use boa_engine::Context;
use boa_engine::JsString;
use boa_engine::JsValue;
use tur_element_tree::DynElement;
use tur_trait::DynElementExt;

pub trait BoaElement: DynElement {
    fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue);
}

pub fn set_element_prop(
    elem: &mut Box<dyn DynElement>,
    ctx: &mut Context,
    key: &JsString,
    value: &JsValue,
) {
    if let Some(e) = elem.cast_mut::<FlexElement>() {
        <dyn BoaElement>::set_prop(e, ctx, key, value);
        return;
    }
    if let Some(e) = elem.cast_mut::<FlexItemElement>() {
        <dyn BoaElement>::set_prop(e, ctx, key, value);
        return;
    }
    if let Some(e) = elem.cast_mut::<StackElement>() {
        <dyn BoaElement>::set_prop(e, ctx, key, value);
        return;
    }
    if let Some(e) = elem.cast_mut::<PositionedElement>() {
        <dyn BoaElement>::set_prop(e, ctx, key, value);
        return;
    }
    if let Some(e) = elem.cast_mut::<ContainerElement>() {
        <dyn BoaElement>::set_prop(e, ctx, key, value);
        return;
    }
    if let Some(e) = elem.cast_mut::<TextElement>() {
        <dyn BoaElement>::set_prop(e, ctx, key, value);
        return;
    }
    tracing::warn!("unknown element type for set_prop: {}", elem.name());
}
