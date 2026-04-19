use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::StackElement;

impl ElementLayout for StackElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let mut max_size = Size::ZERO;

        for &child_id in children {
            let child_constraints = match self.fit {
                tur_shared::StackFit::Loose => Constraints::loose(
                    constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
                ),
                tur_shared::StackFit::Expand => Constraints::tight(
                    constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
                ),
                tur_shared::StackFit::Passthrough => *constraints,
            };
            let size = cx.layout_child(child_id, &child_constraints);
            max_size = Size::new(
                max_size.width.max(size.width),
                max_size.height.max(size.height),
            );
        }

        constraints.constrain(max_size)
    }

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        for &child_id in children {
            let is_positioned = cx.child_type_name(child_id) == "tur_positioned";

            if !is_positioned {
                cx.set_child_offset(child_id, Offset::ZERO);
            }
        }
    }
}

impl ElementRender for StackElement {
    fn type_name(&self) -> &'static str {
        "tur_stack"
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
