//! `Table` — a non-scrollable data table with shared column geometry.
//!
//! The table declares `columns` (fixed `width` px and/or `flex` share of the
//! leftover width — CSS `table-layout: fixed` semantics), a reactive `rows`
//! array, a per-row `build` fn returning that row's cells, and an optional
//! `buildHeader` fn returning the header cells. The header row and every
//! body row lay their cells at the same resolved column widths — the
//! property plain flex composition can't express.
//!
//! Rows are mounted eagerly (all of them — this is the static sibling of a
//! future virtualized table; wrap in a `ScrollView` to scroll). Writing a
//! new array value to the `rows` atom rebuilds the row subtrees (the `Each`
//! rebuild-all semantic) during the next layout pass; the header is built
//! once at `View::build` (reactive header *content* flows through `Val`
//! props inside the returned cells).
//!
//! Chrome: the table paints odd-row `stripeColor` and horizontal
//! `dividerColor` rules (between body rows + under the header) under its
//! cells — pure decoration, no hit-test surface.

pub mod bridge;
mod element;
mod layout;
mod render;

pub use element::{TableColumnDef, TableElement, TableView};

/// Resolve the width of every column against the available (max) width.
/// Pure function — no layout side effects.
///
/// - Columns with a fixed `width` take it.
/// - The leftover space (`available − Σ fixed`) is split among the remaining
///   columns proportionally to their flex weight (a column with neither
///   `width` nor `flex` defaults to flex 1).
/// - Each distributed share is clamped up to the column's `minWidth` (which
///   may push the total beyond `available` — the table box follows the
///   incoming constraints; the cells simply paint at their laid positions).
/// - An unbounded available width (exotic parents only — a vertical
///   `ScrollView` fixes the cross axis) collapses flex columns to their
///   `minWidth`, so the table stays content-sized.
pub(crate) fn resolve_column_widths(available_width: f64, columns: &[TableColumnDef]) -> Vec<f64> {
    let mut fixed_total = 0.0f64;
    let mut flex_total = 0.0f64;
    for c in columns {
        if let Some(w) = c.width {
            fixed_total += w.max(0.0);
        }
        flex_total += c.flex_weight();
    }

    let leftover = if available_width.is_finite() {
        (available_width - fixed_total).max(0.0)
    } else {
        0.0
    };

    columns
        .iter()
        .map(|c| {
            if let Some(w) = c.width {
                w.max(0.0)
            } else {
                let share = if flex_total > 0.0 {
                    leftover * c.flex_weight() / flex_total
                } else {
                    0.0
                };
                let min = c.min_width.unwrap_or(0.0).max(0.0);
                share.max(min)
            }
        })
        .collect()
}

/// The x offset of each column's leading edge (prefix sums of the widths).
pub(crate) fn column_x_offsets(widths: &[f64]) -> Vec<f64> {
    let mut xs = Vec::with_capacity(widths.len());
    let mut x = 0.0;
    for &w in widths {
        xs.push(x);
        x += w;
    }
    xs
}
