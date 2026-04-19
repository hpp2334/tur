use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::render::{ChildLayout, ChildPaint, PaintContext};
use crate::core::traits::{ElementLayout, ElementNodeId, ElementRender};

use super::element::FlexItemElement;

impl ElementLayout for FlexItemElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        child_layout: &mut dyn ChildLayout,
    ) -> Size {
        if let Some(&child_id) = children.first() {
            child_layout.layout_child(child_id, constraints)
        } else {
            constraints.constrain(Size::ZERO)
        }
    }

    fn perform_layout_position(
        &mut self,
        _children: &[ElementNodeId],
        _child_layout: &mut dyn ChildLayout,
    ) {
    }
}

impl ElementRender for FlexItemElement {
    fn type_name(&self) -> &'static str {
        "tur_flex_item"
    }

    fn paint(
        &self,
        _ctx: &mut dyn PaintContext,
        offset: Offset,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        child_paint: &mut dyn ChildPaint,
    ) {
        for &child_id in children {
            child_paint.paint_child(child_id, _ctx, offset);
        }
    }
}
