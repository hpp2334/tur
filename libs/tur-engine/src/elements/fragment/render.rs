use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::FragmentElement;

// FragmentElement is a transparent pass-through with multiple children: it relays
// the incoming constraints to every child, sizes itself to the union (max)
// of the child sizes, positions all children at the origin, and paints
// nothing itself — it only forwards to children.

impl ElementLayout for FragmentElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let mut max_w = 0.0f64;
        let mut max_h = 0.0f64;
        for &child_id in children {
            let s = cx.layout_child(child_id, constraints);
            if s.width > max_w {
                max_w = s.width;
            }
            if s.height > max_h {
                max_h = s.height;
            }
        }
        constraints.constrain(Size::new(max_w, max_h))
    }

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        for &child_id in children {
            cx.set_child_offset(child_id, Offset::ZERO);
        }
    }
}

impl ElementRender for FragmentElement {
    fn type_name(&self) -> &'static str {
        "tur_fragment"
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
