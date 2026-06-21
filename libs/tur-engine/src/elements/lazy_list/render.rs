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

        if self.item_count() == 0 || children.is_empty() {
            self.position.apply_dimensions(viewport, Size::ZERO);
            self.position.set_extents(0.0, 0.0);
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

        self.child_extents.clear();
        self.child_extents.reserve(children.len());

        let mut measured_main = 0.0f64;
        for &child_id in children {
            let size = cx.layout_child(child_id, &child_cs);
            let extent = self.axis.main(size);
            self.child_extents.push(extent);
            measured_main += extent;
        }

        // Total content length is computed from the declared item count,
        // not just the mounted children — so the scrollbar reflects all N
        // items even though only K are mounted. For fixed `itemExtent`,
        // this is exact; for variable heights, we extrapolate from the
        // average of measured children.
        let avg = self.average_extent();
        let total_main = if children.len() as u64 >= self.item_count() {
            // All items are mounted — use the exact sum.
            measured_main
        } else if self.child_extents.is_empty() {
            self.item_count() as f64 * avg
        } else {
            // Average over measured children, scaled to declared count.
            // Use the item_extent directly when provided so the math is exact.
            self.item_count() as f64 * avg
        };

        let content = match self.axis {
            tur_shared::Axis::Vertical => Size::new(viewport.width, total_main),
            tur_shared::Axis::Horizontal => Size::new(total_main, viewport.height),
        };
        self.position.apply_dimensions(viewport, content);
        let viewport_main = self.axis.main(viewport);
        self.last_viewport_main = viewport_main;
        let max_scroll = (total_main - viewport_main).max(0.0);
        self.position.set_extents(0.0, max_scroll);

        viewport
    }

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        let scroll_offset = self.position.pixels();
        // Position each child by its logical item index, not its position
        // in the children slice. This makes layout robust against the
        // parent's children vector being scrambled when items mount out of
        // order during scroll-up, and correctly offsets the first mounted
        // item when the user has scrolled past item 0.
        //
        // For fixed `itemExtent`, content_pos = index * extent is exact.
        // For variable heights, we approximate using the running average
        // (Bug 7 in the design doc — proper fix is a per-index extent
        // cache, deferred).
        let extent = self.average_extent();
        for &child_id in children {
            let Some(logical) = self.visible_index_of(child_id) else {
                continue;
            };
            let content_pos = logical as f64 * extent;
            let main_pos = content_pos - scroll_offset;
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
