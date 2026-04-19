use tur_shared::{ComputedLayout, Constraints, EdgeInsets, Offset, Size};

use crate::core::render::{ChildLayout, ChildPaint, PaintContext};
use crate::core::traits::{ElementLayout, ElementNodeId, ElementRender};

use super::element::ContainerElement;

impl ElementLayout for ContainerElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        child_layout: &mut dyn ChildLayout,
    ) -> Size {
        let sized_constraints = Constraints {
            min_width: self.width.unwrap_or(constraints.min_width),
            max_width: self.width.unwrap_or(constraints.max_width),
            min_height: self.height.unwrap_or(constraints.min_height),
            max_height: self.height.unwrap_or(constraints.max_height),
        };

        let padding = self.padding.map(EdgeInsets::all);
        let inner_constraints = match padding {
            Some(p) => sized_constraints.deflate(p),
            None => sized_constraints,
        };

        let child_size = if let Some(&child_id) = children.first() {
            child_layout.layout_child(child_id, &inner_constraints)
        } else {
            inner_constraints.constrain(Size::ZERO)
        };

        let inflated = match padding {
            Some(p) => p.inflate_size(child_size),
            None => child_size,
        };

        sized_constraints.constrain(inflated)
    }

    fn perform_layout_position(
        &mut self,
        _children: &[ElementNodeId],
        _child_layout: &mut dyn ChildLayout,
    ) {
    }
}

impl ElementRender for ContainerElement {
    fn type_name(&self) -> &'static str {
        "tur_container"
    }

    fn paint(
        &self,
        ctx: &mut dyn PaintContext,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        child_paint: &mut dyn ChildPaint,
    ) {
        if let Some(ref color) = self.color {
            ctx.fill_rect(offset, layout.size, color);
        }
        for &child_id in children {
            child_paint.paint_child(child_id, ctx, offset);
        }
    }
}
