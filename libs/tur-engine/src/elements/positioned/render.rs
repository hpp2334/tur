use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::Positioned;

impl ElementLayout for Positioned {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let left = cx.read_val_opt(self.spec.left.as_ref());
        let top = cx.read_val_opt(self.spec.top.as_ref());
        let right = cx.read_val_opt(self.spec.right.as_ref());
        let bottom = cx.read_val_opt(self.spec.bottom.as_ref());
        let width = cx.read_val_opt(self.spec.width.as_ref());
        let height = cx.read_val_opt(self.spec.height.as_ref());

        // Resolve each axis independently: explicit size wins; otherwise a
        // pair of opposing edges implies a tight extent; else loose.
        let tight_w = width.or_else(|| match (left, right) {
            (Some(l), Some(r)) => Some((constraints.max_width - l - r).max(0.0)),
            _ => None,
        });
        let tight_h = height.or_else(|| match (top, bottom) {
            (Some(t), Some(b)) => Some((constraints.max_height - t - b).max(0.0)),
            _ => None,
        });

        let child_constraints = match (tight_w, tight_h) {
            (Some(w), Some(h)) => Constraints::tight(Size::new(w, h)),
            (Some(w), None) => Constraints {
                min_width: w,
                max_width: w,
                min_height: 0.0,
                max_height: constraints.max_height,
            },
            (None, Some(h)) => Constraints {
                min_width: 0.0,
                max_width: constraints.max_width,
                min_height: h,
                max_height: h,
            },
            (None, None) => Constraints::loose(
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
        let offset_x = cx.read_val_opt(self.spec.left.as_ref()).unwrap_or(0.0);
        let offset_y = cx.read_val_opt(self.spec.top.as_ref()).unwrap_or(0.0);
        cx.set_child_offset_self(Offset::new(offset_x, offset_y));
    }
}

impl ElementRender for Positioned {
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
