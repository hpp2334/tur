use crate::core::element::{ElementNodeId, NodeId};
use crate::core::layout::{Constraints, Offset, Size};
use crate::core::layout::{ElementLayout, LayoutContext, LayoutViewCx};
use crate::core::view::ViewCx;

use super::element::{TableElement, array_len, build_all_rows};
use super::{column_x_offsets, resolve_column_widths};

impl ElementLayout for TableElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        // --- Reconcile rows: rebuild the row subtrees when the `rows`
        // atom's array value changed (new array object or a length change —
        // the Each rebuild-all semantic, executed during layout through a
        // `LayoutViewCx` so newly built cells measure in this same pass).
        // The build fn needs the JS `Context`, borrowed here from the
        // layout's read-only JS face (disjoint from the tree borrow).
        // Scoped so `cx.layout_child` can reborrow the tree below. ---
        {
            let boa = cx.js.boa_mut();
            let mut vcx = LayoutViewCx::new(
                cx.tree,
                cx.node_tree.clone(),
                cx.mutation_queue.clone(),
                cx.dirty.clone(),
            );
            let raw = crate::core::view::read_atom_raw(&vcx, self.view.rows, boa);
            let len = array_len(&raw, boa);
            let changed =
                self.rows_stamp.as_ref().is_none_or(|prev| *prev != raw) || self.rows_len != len;
            if changed {
                for row in std::mem::take(&mut self.row_cells) {
                    for (_, cell) in row {
                        vcx.destroy_child(cell);
                    }
                }
                self.row_cells =
                    build_all_rows(&self.view, &raw, boa, &mut vcx, NodeId::from(self.node_id));
                self.rows_stamp = Some(raw);
                self.rows_len = len;
            }
        }

        // --- Resolve reactive sizing + chrome props (layout holds the
        // store; paint reads `self.painting` and never touches it). ---
        let row_extent = cx.read_val_opt(self.view.row_extent.as_ref());
        let header_extent = cx.read_val_opt(self.view.header_extent.as_ref());
        let row_spacing = cx
            .read_val_opt(self.view.row_spacing.as_ref())
            .unwrap_or(0.0);
        self.painting.stripe = cx.read_val_opt(self.view.stripe_color.as_ref());
        self.painting.divider = cx.read_val_opt(self.view.divider_color.as_ref());
        self.painting.divider_thickness = cx
            .read_val_opt(self.view.divider_thickness.as_ref())
            .unwrap_or(1.0);
        self.painting.header_extent = header_extent;
        self.painting.row_extent = row_extent;

        // --- Shared column geometry. ---
        self.col_widths = resolve_column_widths(constraints.max_width, &self.view.columns);
        let xs = column_x_offsets(&self.col_widths);

        // --- Header: cells at tight column widths; tight extent when given,
        // else the max intrinsic cell height. ---
        self.header_height = 0.0;
        for &(col, cell) in self.header_cells.iter() {
            let cell_id = ElementNodeId::new(cell.as_u64());
            let w = self.col_widths.get(col).copied().unwrap_or(0.0);
            let cs = cell_constraints(w, header_extent);
            let size = cx.layout_child(cell_id, &cs);
            self.header_height = self.header_height.max(size.height);
            cx.set_child_offset(
                cell_id,
                Offset::new(xs.get(col).copied().unwrap_or(0.0), 0.0),
            );
        }
        if let Some(ext) = header_extent {
            self.header_height = ext;
        }

        // --- Body rows: same column constraints per cell; rows stack below
        // the header with `rowSpacing` between them (not after the last). ---
        self.row_heights.clear();
        self.row_tops.clear();
        let mut y = self.header_height;
        for (r, row) in self.row_cells.iter().enumerate() {
            if r > 0 {
                y += row_spacing;
            }
            self.row_tops.push(y);
            let mut intrinsic = 0.0f64;
            for &(col, cell) in row.iter() {
                let cell_id = ElementNodeId::new(cell.as_u64());
                let w = self.col_widths.get(col).copied().unwrap_or(0.0);
                let cs = cell_constraints(w, row_extent);
                let size = cx.layout_child(cell_id, &cs);
                intrinsic = intrinsic.max(size.height);
                cx.set_child_offset(cell_id, Offset::new(xs.get(col).copied().unwrap_or(0.0), y));
            }
            let row_h = row_extent.unwrap_or(intrinsic);
            self.row_heights.push(row_h);
            y += row_h;
        }

        // --- Own size: fill the available width when bounded (stripe /
        // divider chrome spans the full table width), else content width;
        // content height. minWidth clamping may overflow `max_width` — the
        // cells simply paint at their laid positions (Grid parity). ---
        let content_w: f64 = self.col_widths.iter().sum();
        let own_w = if constraints.max_width.is_finite() {
            constraints.max_width.max(content_w)
        } else {
            content_w
        };
        let size = constraints.constrain(Size::new(own_w, y));
        self.painting.width = size.width;
        size
    }
}

/// Constraints for one cell: tight width (the column's resolved width),
/// tight height when an extent is given (cells fill it), else loose height
/// (content-sized; the row height is the max intrinsic cell height).
fn cell_constraints(width: f64, extent: Option<f64>) -> Constraints {
    match extent {
        Some(ext) => Constraints {
            min_width: width,
            max_width: width,
            min_height: ext,
            max_height: ext,
        },
        None => Constraints {
            min_width: width,
            max_width: width,
            min_height: 0.0,
            max_height: f64::INFINITY,
        },
    }
}
