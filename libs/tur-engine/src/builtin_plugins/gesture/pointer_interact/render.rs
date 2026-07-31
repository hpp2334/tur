use crate::core::layout::ComputedLayout;

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::PointerInteractElement;

impl ElementRender for PointerInteractElement {
    fn type_name(&self) -> &'static str {
        "tur_pointer_interact"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas);
        }
    }
}
