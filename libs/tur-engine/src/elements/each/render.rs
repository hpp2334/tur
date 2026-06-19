use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::EachElement;

// `EachElement` is a transparent relay to a `FlexElement` layout: it forwards the incoming
// constraints to its mounted item children (laid out as a vertical flex via
// the `FlexElement` delegate held on the element), positions them, and paints
// nothing itself.

impl ElementLayout for EachElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        self.flex.perform_layout_size(constraints, children, cx)
    }

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        self.flex.perform_layout_position(children, cx);
    }
}

impl ElementRender for EachElement {
    fn type_name(&self) -> &'static str {
        "tur_each"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas, offset);
        }
    }
}
