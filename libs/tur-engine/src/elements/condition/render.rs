use tur_shared::{ComputedLayout, Offset};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::ConditionElement;

// ConditionElement is a transparent pass-through: it relays constraints to its single
// mounted child, takes the child's size, and positions the child at the origin.
// It paints nothing itself — it only forwards to children.

impl ElementRender for ConditionElement {
    fn type_name(&self) -> &'static str {
        "tur_condition"
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
