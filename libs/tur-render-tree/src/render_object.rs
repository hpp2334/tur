use std::fmt;

use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::RenderNodeId;

pub trait RenderObject: fmt::Debug + Send + Sync {
    fn type_name(&self) -> &'static str;

    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[RenderNodeId],
        child_layout: &mut dyn ChildLayout,
    ) -> Size;

    fn perform_layout_position(
        &mut self,
        children: &[RenderNodeId],
        child_layout: &mut dyn ChildLayout,
    );

    fn paint(
        &self,
        ctx: &mut dyn PaintContext,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[RenderNodeId],
        child_paint: &mut dyn ChildPaint,
    );

    fn hit_test(&self, position: Offset, layout: &ComputedLayout) -> bool {
        position.x >= 0.0
            && position.x < layout.size.width
            && position.y >= 0.0
            && position.y < layout.size.height
    }
}

pub trait ChildLayout {
    fn layout_child(&mut self, child_id: RenderNodeId, constraints: &Constraints) -> Size;
    fn set_child_offset(&mut self, child_id: RenderNodeId, offset: Offset);
    fn set_child_offset_self(&mut self, offset: Offset);
    fn get_child_type_name(&self, child_id: RenderNodeId) -> &'static str;
}

pub trait ChildPaint {
    fn paint_child(
        &mut self,
        child_id: RenderNodeId,
        ctx: &mut dyn PaintContext,
        parent_offset: Offset,
    );
}

pub trait PaintContext: fmt::Debug {
    fn fill_rect(&mut self, offset: Offset, size: Size, color: &str);
    fn fill_text(&mut self, offset: Offset, text: &str, font_size: f64, color: &str);
}
