use crate::core::layout::{Constraints, Offset, Size};
use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::GridElement;
use super::{compute_grid_metrics, cross_offset};

impl ElementLayout for GridElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let max_cross_axis_extent = cx.read_val(&self.view.max_cross_axis_extent).unwrap_or(100.0);
        let child_aspect_ratio = cx.read_val_opt(self.view.child_aspect_ratio.as_ref());
        let main_axis_extent = cx.read_val_opt(self.view.main_axis_extent.as_ref());
        let cross_axis_spacing = cx.read_val_opt(self.view.cross_axis_spacing.as_ref()).unwrap_or(0.0);
        let main_axis_spacing = cx.read_val_opt(self.view.main_axis_spacing.as_ref()).unwrap_or(0.0);

        // Grid is vertical-flow: cross axis = width, main axis = height.
        let cross_axis_size = constraints.max_width;

        let metrics = compute_grid_metrics(
            cross_axis_size,
            max_cross_axis_extent,
            cross_axis_spacing,
            main_axis_spacing,
            child_aspect_ratio,
            main_axis_extent,
        );
        self.metrics = Some(metrics);
        self.constraints = Some(*constraints);

        let count = metrics.cross_axis_count.max(1);
        let child_constraints = Constraints::tight(Size::new(metrics.cell_cross, metrics.cell_main));

        // Single-pass measure + position: lay each child out at its tight cell
        // size and assign the row-major offset.
        for (i, &child_id) in children.iter().enumerate() {
            let _ = cx.layout_child(child_id, &child_constraints);
            let (row, col) = (i / count, i % count);
            let x = cross_offset(col, metrics.cell_cross, cross_axis_spacing);
            let y = row as f64 * metrics.stride_main;
            cx.set_child_offset(child_id, Offset::new(x, y));
        }

        // Total content extent along the main axis: rows * stride minus the
        // trailing spacing (no gap after the last row).
        let rows = children.len().div_ceil(count);
        let total_main = if children.is_empty() {
            0.0
        } else {
            rows as f64 * metrics.stride_main - main_axis_spacing
        };

        // Own cross-axis size: fill the available width when bounded,
        // otherwise the content width.
        let own_cross = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            count as f64 * metrics.cell_cross + (count.saturating_sub(1) as f64) * cross_axis_spacing
        };

        let size = Size::new(own_cross, total_main);
        let final_size = constraints.constrain(size);
        self.computed_size = Some(final_size);
        self.overflow = (total_main - final_size.height).max(0.0);

        final_size
    }
}
