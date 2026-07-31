use crate::core::layout::{ComputedLayout, Geometry, Offset};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::ScrollViewElement;

impl ElementRender for ScrollViewElement {
    fn type_name(&self) -> &'static str {
        "tur_scroll_view"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        if let Some(ref brush) = self.painting.color {
            canvas.fill_geometry(Offset::ZERO, &Geometry::Rect(layout.size), brush);
        }

        canvas.push_clip(Offset::ZERO, layout.size);
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas);
        }
        canvas.pop_clip();
    }
}
