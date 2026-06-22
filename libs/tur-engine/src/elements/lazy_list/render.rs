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
            // Persist the measurement in the per-index cache. Keyed by
            // logical index (stable across scroll-driven mount/unmount) so
            // unmounted-then-remounted items recall their previous extent.
            if let Some(logical) = self.visible_index_of(child_id) {
                self.extent_cache.insert(logical, extent);
            }
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

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], cx: &mut LayoutContext) {
        let scroll_offset = self.position.pixels();
        let avg = self.average_extent();

        // Position each child by its logical item index, not its position
        // in the children slice. This makes layout robust against the
        // parent's children vector being scrambled when items mount out of
        // order during scroll-up.
        //
        // For BOTH fixed `itemExtent` and variable heights, position each
        // mounted child at the cumulative offset of its logical index
        // (sum of extents of all previous items). The cache makes this
        // exact for previously-measured items; the avg fallback covers
        // the (typically pre-scroll) region before the first mounted item.
        //
        // Fast path when items 0..first_mounted are all unmeasured: skip
        // the walk and use `first_mounted * avg`. This keeps deep-scroll
        // layouts O(visible_count) instead of O(first_mounted).
        let visible: Vec<(u64, ElementNodeId)> = self.visible.clone();
        let first_mounted_idx = visible.first().map(|(i, _)| *i).unwrap_or(0);

        let mut offset = {
            let all_unmeasured = (0..first_mounted_idx)
                .all(|i| !self.extent_cache.contains_key(&i));
            if all_unmeasured {
                first_mounted_idx as f64 * avg
            } else {
                self.cumulative_offset(first_mounted_idx)
            }
        };

        for (i, child_id) in visible {
            let main_pos = offset - scroll_offset;
            let off = match self.axis {
                tur_shared::Axis::Vertical => Offset::new(0.0, main_pos),
                tur_shared::Axis::Horizontal => Offset::new(main_pos, 0.0),
            };
            cx.set_child_offset(child_id, off);
            // Advance by this item's extent (cached measurement or avg).
            offset += self.extent_cache.get(&i).copied().unwrap_or(avg);
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
