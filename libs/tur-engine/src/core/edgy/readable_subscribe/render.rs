use crate::core::layout::ComputedLayout;

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::ReadableSubscribeElement;

// Transparent pass-through: paint the child subtree unchanged.
impl ElementRender for ReadableSubscribeElement {
    fn type_name(&self) -> &'static str {
        "tur_readable_subscribe"
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
