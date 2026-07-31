use crate::core::layout::{ComputedLayout, Offset};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::LazyListElement;

impl ElementRender for LazyListElement {
    fn type_name(&self) -> &'static str {
        "tur_lazy_list"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        canvas.push_clip(Offset::ZERO, layout.size);
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas);
        }
        canvas.pop_clip();
    }
}
