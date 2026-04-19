use tur_shared::{Constraints, Offset, Size};

use crate::{ChildLayout, ChildPaint, ComputedLayout, PaintContext, RenderNodeId, RenderObject};

#[derive(Debug)]
pub struct FlexItemRenderObject;

impl RenderObject for FlexItemRenderObject {
    fn type_name(&self) -> &'static str {
        "tur_flex_item"
    }

    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[RenderNodeId],
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
        _children: &[RenderNodeId],
        _child_layout: &mut dyn ChildLayout,
    ) {
    }

    fn paint(
        &self,
        _ctx: &mut dyn PaintContext,
        _offset: Offset,
        _layout: &ComputedLayout,
        children: &[RenderNodeId],
        child_paint: &mut dyn ChildPaint,
    ) {
        for &child_id in children {
            child_paint.paint_child(child_id, _ctx, _offset);
        }
    }
}
