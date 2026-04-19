use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::render::{ChildLayout, ChildPaint, PaintContext};
use crate::core::traits::{ElementLayout, ElementNodeId, ElementRender};

use super::element::StackElement;

impl ElementLayout for StackElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        child_layout: &mut dyn ChildLayout,
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
            let size = child_layout.layout_child(child_id, &child_constraints);
            max_size = Size::new(
                max_size.width.max(size.width),
                max_size.height.max(size.height),
            );
        }

        constraints.constrain(max_size)
    }

    fn perform_layout_position(
        &mut self,
        children: &[ElementNodeId],
        child_layout: &mut dyn ChildLayout,
    ) {
        for &child_id in children {
            let is_positioned = child_layout.get_child_type_name(child_id) == "tur_positioned";

            if !is_positioned {
                child_layout.set_child_offset(child_id, Offset::ZERO);
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
        _ctx: &mut dyn PaintContext,
        offset: Offset,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        child_paint: &mut dyn ChildPaint,
    ) {
        for &child_id in children {
            child_paint.paint_child(child_id, _ctx, offset);
        }
    }
}
