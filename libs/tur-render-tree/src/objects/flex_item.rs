use tur_shared::{ComputedLayout, Constraints, ElementKind, Offset, Size};

use crate::render_object::{ChildLayout, ChildPaint, PaintContext, RenderObject};
use crate::RenderNodeId;

#[derive(Debug)]
pub struct FlexItemRenderObject;

impl RenderObject for FlexItemRenderObject {
    fn kind(&self) -> ElementKind {
        ElementKind::FlexItem
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
