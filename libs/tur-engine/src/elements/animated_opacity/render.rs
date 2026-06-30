use tur_shared::{ComputedLayout, Offset};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::AnimatedOpacityElement;

impl ElementRender for AnimatedOpacityElement {
    fn type_name(&self) -> &'static str {
        "tur_animated_opacity"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        canvas.push_opacity(self.painting);
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas, offset);
        }
        canvas.pop_opacity();
    }
}
