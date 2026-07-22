//! `Grid` — a non-scrollable layout element that tiles its static children
//! into a row-major grid. The cross-axis (column) count is derived from a
//! max cross-axis extent: `count = floor(crossAxisSize / maxCrossAxisExtent)`.
//!
//! Cell sizes are uniform: the main-axis (row) size is either a fixed
//! `mainAxisExtent` or derived from `childAspectRatio` (`cell_cross / ratio`),
//! defaulting to square when neither is given. This keeps sizing deterministic
//! (no per-cell measurement), so the same metrics math is reused by
//! [`LazyGrid`](crate::builtin_plugins::lazy_container::lazy_grid) for exact
//! virtualization.

mod element;
mod layout;
mod render;
pub mod bridge;

pub use element::{GridElement, GridView};

/// Computed grid geometry shared by `Grid` and `LazyGrid`.
///
/// All quantities are in content space. `stride_main` is the row pitch
/// (`cell_main + main_axis_spacing`) — the per-row advance along the main axis.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GridMetrics {
    /// Number of cells per cross-axis line (column count for a vertical grid).
    pub cross_axis_count: usize,
    /// Size of each cell along the cross axis.
    pub cell_cross: f64,
    /// Size of each cell along the main axis.
    pub cell_main: f64,
    /// Row pitch: `cell_main + main_axis_spacing`.
    pub stride_main: f64,
}

/// Compute grid geometry from the available cross-axis size and the grid
/// props. Pure function — no layout side effects.
///
/// When `cross_axis_size` is unbounded (infinite), the column count falls
/// back to 1 and each cell gets `max_cross_axis_extent` as its cross size so
/// the layout stays finite.
pub(crate) fn compute_grid_metrics(
    cross_axis_size: f64,
    max_cross_axis_extent: f64,
    cross_axis_spacing: f64,
    main_axis_spacing: f64,
    child_aspect_ratio: Option<f64>,
    main_axis_extent: Option<f64>,
) -> GridMetrics {
    let bounded = cross_axis_size.is_finite() && cross_axis_size > 0.0;

    let cross_axis_count = if bounded && max_cross_axis_extent > 0.0 {
        ((cross_axis_size / max_cross_axis_extent).floor() as usize).max(1)
    } else {
        1
    };

    let cell_cross = if bounded {
        let gap_total = (cross_axis_count.saturating_sub(1) as f64) * cross_axis_spacing;
        ((cross_axis_size - gap_total).max(0.0)) / cross_axis_count as f64
    } else {
        // Unbounded cross axis: use the max extent itself so children stay finite.
        max_cross_axis_extent.max(0.0)
    };

    let cell_main = if let Some(ext) = main_axis_extent {
        ext.max(0.0)
    } else {
        let ratio = child_aspect_ratio.unwrap_or(1.0);
        if ratio > 0.0 {
            cell_cross / ratio
        } else {
            cell_cross
        }
    };

    // The cross-axis pitch (used by LazyGrid to advance cell x positions).
    let stride_main = cell_main + main_axis_spacing;

    GridMetrics {
        cross_axis_count,
        cell_cross,
        cell_main,
        stride_main,
    }
}

/// Cross-axis offset (x for a vertical grid) of column `col`.
pub(crate) fn cross_offset(col: usize, cell_cross: f64, cross_axis_spacing: f64) -> f64 {
    col as f64 * (cell_cross + cross_axis_spacing)
}
