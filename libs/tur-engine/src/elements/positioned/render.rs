use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::render::{Canvas, LayoutContext, PaintContext};
use crate::core::traits::{ElementLayout, ElementNodeId, ElementRender};

use super::element::PositionedElement;

impl ElementLayout for PositionedElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let child_constraints = match (self.left, self.right, self.top, self.bottom) {
            (Some(_), Some(_), Some(_), Some(_)) => {
                let w =
                    (constraints.max_width - self.left.unwrap_or(0.0) - self.right.unwrap_or(0.0))
                        .max(0.0);
                let h =
                    (constraints.max_height - self.top.unwrap_or(0.0) - self.bottom.unwrap_or(0.0))
                        .max(0.0);
                Constraints::tight(Size::new(w, h))
            }
            _ => Constraints::loose(
                constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
            ),
        };

        if let Some(&child_id) = children.first() {
            cx.layout_child(child_id, &child_constraints)
        } else {
            child_constraints.constrain(Size::ZERO)
        }
    }

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], cx: &mut LayoutContext) {
        let offset_x = self.left.unwrap_or(0.0);
        let offset_y = self.top.unwrap_or(0.0);
        cx.set_child_offset_self(Offset::new(offset_x, offset_y));
    }
}

impl ElementRender for PositionedElement {
    fn type_name(&self) -> &'static str {
        "tur_positioned"
    }

    fn paint(
        &self,
        _canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        for &child_id in children {
            paint_ctx.paint_child(child_id, _canvas, offset);
        }
    }
}
