use crate::core::layout::ComputedLayout;

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::ExpandedElement;

impl ElementRender for ExpandedElement {
    fn type_name(&self) -> &'static str {
        "tur_flex_item"
    }

    fn paint(
        &self,
        _canvas: &mut dyn Canvas,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        for &child_id in children {
            paint_ctx.paint_child(child_id, _canvas);
        }
    }
}
