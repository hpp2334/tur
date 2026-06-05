use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::LazyListElement;

impl ElementLayout for LazyListElement {
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

        if self.item_extent <= 0.0 || self.item_count == 0 {
            self.position.apply_dimensions(viewport, Size::ZERO);
            self.position.set_extents(0.0, 0.0);
            self.update_controller_metrics();
            return viewport;
        }

        let total_main = self.item_count as f64 * self.item_extent;
        let content = match self.axis {
            tur_shared::Axis::Vertical => Size::new(viewport.width, total_main),
            tur_shared::Axis::Horizontal => Size::new(total_main, viewport.height),
        };
        self.position.apply_dimensions(viewport, content);
        let max_scroll = (total_main - self.axis.main(viewport)).max(0.0);
        self.position.set_extents(0.0, max_scroll);

        let child_cs = match self.axis {
            tur_shared::Axis::Vertical => Constraints {
                min_width: viewport.width,
                max_width: viewport.width,
                min_height: self.item_extent,
                max_height: self.item_extent,
            },
            tur_shared::Axis::Horizontal => Constraints {
                min_width: self.item_extent,
                max_width: self.item_extent,
                min_height: viewport.height,
                max_height: viewport.height,
            },
        };

        for &child_id in children {
            cx.layout_child(child_id, &child_cs);
        }

        self.update_controller_metrics();
        viewport
    }

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        let scroll_offset = self.position.pixels();
        for (i, &child_id) in children.iter().enumerate() {
            let logical_index = (self.start_index + i as u64) as f64;
            let main_pos = logical_index * self.item_extent - scroll_offset;
            let offset = match self.axis {
                tur_shared::Axis::Vertical => Offset::new(0.0, main_pos),
                tur_shared::Axis::Horizontal => Offset::new(main_pos, 0.0),
            };
            cx.set_child_offset(child_id, offset);
        }
    }
}

impl ElementRender for LazyListElement {
    fn type_name(&self) -> &'static str {
        "tur_lazy_list"
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
