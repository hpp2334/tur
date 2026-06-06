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

        if self.item_count == 0 {
            self.position.apply_dimensions(viewport, Size::ZERO);
            self.position.set_extents(0.0, 0.0);
            self.update_controller_metrics();
            return viewport;
        }

        let child_cs = match self.axis {
            tur_shared::Axis::Vertical => Constraints {
                min_width: viewport.width,
                max_width: viewport.width,
                min_height: 0.0,
                max_height: f64::INFINITY,
            },
            tur_shared::Axis::Horizontal => Constraints {
                min_width: 0.0,
                max_width: f64::INFINITY,
                min_height: viewport.height,
                max_height: viewport.height,
            },
        };

        let mut measured_sum = 0.0f64;
        let mut measured_count = 0u64;

        self.child_extents.clear();
        self.child_extents.reserve(children.len());

        for &child_id in children {
            let size = cx.layout_child(child_id, &child_cs);
            let extent = self.axis.main(size);
            self.child_extents.push(extent);
            measured_sum += extent;
            measured_count += 1;
        }

        if measured_count > 0 {
            self.measured_total_sum += measured_sum;
            self.measured_count += measured_count;
        }

        let total_main = self.estimate_total_extent();

        let content = match self.axis {
            tur_shared::Axis::Vertical => Size::new(viewport.width, total_main),
            tur_shared::Axis::Horizontal => Size::new(total_main, viewport.height),
        };
        self.position.apply_dimensions(viewport, content);
        let max_scroll = (total_main - self.axis.main(viewport)).max(0.0);
        self.position.set_extents(0.0, max_scroll);

        self.update_controller_metrics();
        viewport
    }

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        let avg = self.average_extent();
        let preceding = self.start_index as f64 * avg;
        let scroll_offset = self.position.pixels();
        let mut cum = preceding;

        for (i, &child_id) in children.iter().enumerate() {
            let extent = self.child_extents.get(i).copied().unwrap_or(avg);
            let main_pos = cum - scroll_offset;
            let offset = match self.axis {
                tur_shared::Axis::Vertical => Offset::new(0.0, main_pos),
                tur_shared::Axis::Horizontal => Offset::new(main_pos, 0.0),
            };
            cx.set_child_offset(child_id, offset);
            cum += extent;
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
