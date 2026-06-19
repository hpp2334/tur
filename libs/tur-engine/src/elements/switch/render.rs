use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::SwitchElement;

// SwitchElement is a transparent pass-through (like ConditionElement): it relays constraints
// to its single mounted child, takes the child's size, positions the child at
// the origin, and paints nothing itself.

impl ElementLayout for SwitchElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        if let Some(&child_id) = children.first() {
            cx.layout_child(child_id, constraints)
        } else {
            constraints.constrain(Size::ZERO)
        }
    }

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        if let Some(&child_id) = children.first() {
            cx.set_child_offset(child_id, Offset::ZERO);
        }
    }
}

impl ElementRender for SwitchElement {
    fn type_name(&self) -> &'static str {
        "tur_switch"
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
