use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::ScrollViewElement;

impl ElementLayout for ScrollViewElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let viewport_w = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            0.0
        };
        let viewport_h = if constraints.max_height.is_finite() {
            constraints.max_height
        } else {
            0.0
        };
        let viewport = constraints.constrain(Size::new(viewport_w, viewport_h));

        if let Some(&child_id) = children.first() {
            let child_constraints = match self.axis {
                tur_shared::Axis::Vertical => Constraints {
                    min_width: constraints.min_width,
                    max_width: constraints.max_width,
                    min_height: 0.0,
                    max_height: f64::INFINITY,
                },
                tur_shared::Axis::Horizontal => Constraints {
                    min_width: 0.0,
                    max_width: f64::INFINITY,
                    min_height: constraints.min_height,
                    max_height: constraints.max_height,
                },
            };
            let child_size = cx.layout_child(child_id, &child_constraints);
            self.position.apply_dimensions(viewport, child_size);
            let max_scroll = (self.axis.main(child_size) - self.axis.main(viewport)).max(0.0);
            self.position.set_extents(0.0, max_scroll);
        } else {
            self.position.apply_dimensions(viewport, tur_shared::Size::ZERO);
            self.position.set_extents(0.0, 0.0);
        }

        viewport
    }

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        if let Some(&child_id) = children.first() {
            let offset = match self.axis {
                tur_shared::Axis::Vertical => Offset::new(0.0, -self.position.pixels()),
                tur_shared::Axis::Horizontal => Offset::new(-self.position.pixels(), 0.0),
            };
            cx.set_child_offset(child_id, offset);
        }
    }
}

impl ElementRender for ScrollViewElement {
    fn type_name(&self) -> &'static str {
        "tur_scroll_view"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        canvas.push_clip(offset, layout.size);
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas, offset);
        }
        canvas.pop_clip();
    }
}
