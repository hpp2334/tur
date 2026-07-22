use crate::core::layout::{Axis, Constraints, Offset, Size};
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::layout::{ElementLayout, LayoutContext, LayoutViewCx};
use crate::core::view::ViewCx;
use crate::builtin_plugins::layout::{compute_grid_metrics, cross_offset};

use super::element::LazyGridElement;

impl ElementLayout for LazyGridElement {
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

        let (cross_axis_size, viewport_main) = match self.axis {
            Axis::Vertical => (viewport.width, viewport.height),
            Axis::Horizontal => (viewport.height, viewport.width),
        };

        if self.item_count() == 0 {
            self.position.apply_dimensions(viewport, Size::ZERO);
            self.position.set_extents(0.0, 0.0);
            if !self.visible.is_empty() {
                let to_destroy: Vec<NodeId> = self.visible.iter().map(|&(_, id)| id).collect();
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

        // Compute geometry from the real viewport.
        let max_cross_axis_extent = cx.read_val(&self.view.max_cross_axis_extent).unwrap_or(100.0);
        let child_aspect_ratio = cx.read_val_opt(self.view.child_aspect_ratio.as_ref());
        let main_axis_extent = cx.read_val_opt(self.view.main_axis_extent.as_ref());
        let cross_axis_spacing = cx.read_val_opt(self.view.cross_axis_spacing.as_ref()).unwrap_or(0.0);
        let main_axis_spacing = cx.read_val_opt(self.view.main_axis_spacing.as_ref()).unwrap_or(0.0);

        let metrics = compute_grid_metrics(
            cross_axis_size,
            max_cross_axis_extent,
            cross_axis_spacing,
            main_axis_spacing,
            child_aspect_ratio,
            main_axis_extent,
        );

        // Cache the freshly-computed geometry. If the column count changed
        // (resize crossing a max-extent boundary), the index→(line,slot)
        // mapping shifts; positioning is analytic so it self-corrects next
        // pass, but we flag a remount to refresh the visible set.
        let count_changed = self.cross_axis_count != metrics.cross_axis_count;
        self.cross_axis_count = metrics.cross_axis_count;
        self.cell_cross = metrics.cell_cross;
        self.cell_main = metrics.cell_main;
        self.stride_main = metrics.stride_main;
        self.cross_axis_spacing = cross_axis_spacing;
        self.main_axis_spacing = main_axis_spacing;

        // --- reactive-change reaction + remount (build-during-layout) ---
        if viewport_main > 0.0 {
            let boa = cx.js.boa_mut();
            let mut vcx = LayoutViewCx::new(
                cx.tree,
                cx.node_tree.clone(),
                cx.mutation_queue.clone(),
                cx.dirty.clone(),
            );
            self.react_to_prop_changes(&mut vcx, boa);
            // On column-count change, unmount everything so the visible set is
            // rebuilt against the new line/slot mapping with no stragglers.
            if count_changed {
                let to_destroy: Vec<NodeId> =
                    self.visible.iter().map(|&(_, id)| id).collect();
                for id in to_destroy {
                    vcx.destroy_child(id);
                }
                self.visible.clear();
            }
            self.remount(&mut vcx, boa, viewport_main);
        }

        // --- measure phase: lay out every currently-mounted cell at its
        // tight uniform cell size. ---
        let child_cs = match self.axis {
            Axis::Vertical => Constraints::tight(Size::new(self.cell_cross, self.cell_main)),
            Axis::Horizontal => Constraints::tight(Size::new(self.cell_main, self.cell_cross)),
        };

        let visible: Vec<(u64, NodeId)> = self.visible.clone();
        for (_logical, child_id) in &visible {
            let _ = cx.layout_child(ElementNodeId::new(child_id.as_u64()), &child_cs);
        }

        // Total content length along the main axis (exact: uniform cells).
        let count = self.cross_axis_count.max(1) as u64;
        let item_count = self.item_count();
        let total_lines = if item_count == 0 { 0 } else { item_count.div_ceil(count) };
        let total_main = if total_lines == 0 {
            0.0
        } else {
            total_lines as f64 * self.stride_main - self.main_axis_spacing
        };

        let content = match self.axis {
            Axis::Vertical => Size::new(viewport.width, total_main),
            Axis::Horizontal => Size::new(total_main, viewport.height),
        };
        self.position.apply_dimensions(viewport, content);
        let max_scroll = (total_main - viewport_main).max(0.0);
        self.position.set_extents(0.0, max_scroll);

        // --- position (assign child offsets) ---
        self.assign_offsets(cx);

        viewport
    }
}

impl LazyGridElement {
    fn assign_offsets(&mut self, cx: &mut LayoutContext) {
        let scroll_offset = self.position.pixels();
        let count = self.cross_axis_count.max(1);
        let cell_cross = self.cell_cross;
        let stride = self.stride_main;
        let cross_spacing = self.cross_axis_spacing;

        let visible: Vec<(u64, NodeId)> = self.visible.clone();
        for (index, child_id) in visible {
            let line = index as usize / count;
            let slot = index as usize % count;
            let main_pos = line as f64 * stride - scroll_offset;
            let cross_pos = cross_offset(slot, cell_cross, cross_spacing);
            let off = match self.axis {
                Axis::Vertical => Offset::new(cross_pos, main_pos),
                Axis::Horizontal => Offset::new(main_pos, cross_pos),
            };
            cx.set_child_offset(ElementNodeId::new(child_id.as_u64()), off);
        }
    }
}
