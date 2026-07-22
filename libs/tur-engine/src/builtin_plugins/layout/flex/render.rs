use crate::core::layout::{ComputedLayout, Offset};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::FlexElement;

impl ElementRender for FlexElement {
    fn type_name(&self) -> &'static str {
        "tur_flex"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        if self.overflow > 0.0 {
            canvas.push_clip(offset, layout.size);
        }
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas, offset);
        }
        if self.overflow > 0.0 {
            canvas.pop_clip();
        }
    }
}
