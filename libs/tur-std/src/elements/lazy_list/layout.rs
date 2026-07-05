use tur_shared::{Constraints, Offset, Size};

use tur_engine::core::element::{ElementNodeId, NodeId};
use tur_engine::core::layout::{ElementLayout, LayoutContext, LayoutViewCx};
use tur_engine::core::view::ViewCx;

use super::element::LazyListElement;

impl ElementLayout for LazyListElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
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
        let viewport_main = self.axis.main(viewport);

        if self.item_count() == 0 {
            self.position.apply_dimensions(viewport, Size::ZERO);
            self.position.set_extents(0.0, 0.0);
            // Drop any mounted items.
            if !self.visible.is_empty() {
                let to_destroy: Vec<NodeId> =
                    self.visible.iter().map(|&(_, id)| id).collect();
                self.visible.clear();
                let mut vcx = LayoutViewCx::new(
                    cx.tree,
                    cx.node_tree.clone(),
                    cx.mutation_queue.clone(),
                    cx.dirty.clone(),
                );
                for id in to_destroy {
                    vcx.destroy_child(id);
                }
            }
            return viewport;
        }

        // --- reactive-change reaction + remount: use the REAL viewport
        // (from constraints) to adjust the mounted set. Build-during-layout
        // runs through a LayoutViewCx backed by this same `&mut NodeTreeData`
        // borrow, so no competing Rc<RefCell> borrow. Dropped before measure
        // so `cx.layout_child` can reborrow the tree. ---
        if viewport_main > 0.0 {
            let boa = cx.js.boa_mut();
            let mut vcx = LayoutViewCx::new(
                cx.tree,
                cx.node_tree.clone(),
                cx.mutation_queue.clone(),
                cx.dirty.clone(),
            );
            // React to axis/itemExtent/itemCount changes first (replaces the
            // former pre-layout Effect handler).
            self.react_to_prop_changes(&mut vcx, boa);
            self.remount(&mut vcx, boa, viewport_main);
        }

        // --- measure phase: lay out every currently-mounted item. We
        // iterate `self.visible` (the authoritative mounted set after
        // remount), not the `children` snapshot, so newly-mounted items
        // measure in this same pass. ---
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
        self.child_extents.reserve(self.visible.len());

        let visible: Vec<(u64, NodeId)> = self.visible.clone();
        let mut measured_main = 0.0f64;
        for (logical, child_id) in &visible {
            let size = cx.layout_child(ElementNodeId::new(child_id.as_u64()), &child_cs);
            let extent = self.axis.main(size);
            self.child_extents.push(extent);
            measured_main += extent;
            // Persist the measurement in the per-index cache. Keyed by
            // logical index (stable across scroll-driven mount/unmount) so
            // unmounted-then-remounted items recall their previous extent.
            self.extent_cache.insert(*logical, extent);
        }

        // Total content length is computed from the declared item count,
        // not just the mounted children — so the scrollbar reflects all N
        // items even though only K are mounted. For fixed `itemExtent`,
        // this is exact; for variable heights, we extrapolate from the
        // average of measured children.
        let avg = self.average_extent();
        let total_main = if visible.len() as u64 >= self.item_count() {
            // All items are mounted — use the exact sum.
            measured_main
        } else {
            self.item_count() as f64 * avg
        };

        let content = match self.axis {
            tur_shared::Axis::Vertical => Size::new(viewport.width, total_main),
            tur_shared::Axis::Horizontal => Size::new(total_main, viewport.height),
        };
        self.position.apply_dimensions(viewport, content);
        let max_scroll = (total_main - viewport_main).max(0.0);
        self.position.set_extents(0.0, max_scroll);

        // --- position (assign child offsets) ---
        self.assign_offsets(cx);

        viewport
    }
}

impl LazyListElement {
    fn assign_offsets(&mut self, cx: &mut LayoutContext) {
        let scroll_offset = self.position.pixels();
        let avg = self.average_extent();

        // Anchor-and-walk positioning: start from the persistent
        // `first_mounted_offset` (content-space y of the first mounted
        // item's top edge) and walk forward, setting each child's offset to
        // the running sum and advancing by the child's cached (or
        // avg-fallback) extent. O(visible_count) per layout, regardless of
        // how deep the user has scrolled.
        //
        // The anchor is maintained in `remount` (delta-updated as the
        // leading visible index shifts) and reset on axis/itemExtent/
        // itemCount changes in `react_to_prop_changes`.
        let visible: Vec<(u64, NodeId)> = self.visible.clone();
        let mut offset = self.first_mounted_offset;
        for (i, child_id) in visible {
            let main_pos = offset - scroll_offset;
            let off = match self.axis {
                tur_shared::Axis::Vertical => Offset::new(0.0, main_pos),
                tur_shared::Axis::Horizontal => Offset::new(main_pos, 0.0),
            };
            cx.set_child_offset(ElementNodeId::new(child_id.as_u64()), off);
            offset += self.extent_cache.get(&i).copied().unwrap_or(avg);
        }
    }
}

