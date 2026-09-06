use crate::core::element::ElementNodeId;
use crate::core::layout::{ComputedLayout, Geometry, Offset, Size};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::TableElement;

impl ElementRender for TableElement {
    fn type_name(&self) -> &'static str {
        "tur_table"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        _layout: &ComputedLayout,
        _children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        let p = &self.painting;

        // Chrome first (under the cells): odd-row stripes, then the
        // horizontal dividers (under the header + between body rows).
        if let Some(brush) = &p.stripe {
            for (i, top) in self.row_tops.iter().enumerate() {
                if i % 2 == 1 {
                    let h = self.row_heights.get(i).copied().unwrap_or(0.0);
                    if h > 0.0 {
                        canvas.fill_geometry(
                            Offset::new(0.0, *top),
                            &Geometry::Rect(Size::new(p.width, h)),
                            brush,
                        );
                    }
                }
            }
        }

        if let Some(brush) = &p.divider
            && p.divider_thickness > 0.0
        {
            let t = p.divider_thickness;
            let rect = Geometry::Rect(Size::new(p.width, t));
            if self.header_height > 0.0 {
                canvas.fill_geometry(Offset::new(0.0, self.header_height), &rect, brush);
            }
            // Between body rows: the top edge of every row after the first.
            for top in self.row_tops.iter().skip(1) {
                canvas.fill_geometry(Offset::new(0.0, *top), &rect, brush);
            }
        }

        // Cells — the element's own bookkeeping is authoritative (the
        // `children` snapshot may predate an in-layout row rebuild). With a
        // fixed extent, each box clips its cells so overflowing content
        // (e.g. wrapped text taller than `rowExtent`) can't bleed into the
        // next row (Flutter's overflow-clipped fixed-extent parity).
        if let Some(ext) = p.header_extent {
            canvas.push_clip(Offset::ZERO, Size::new(p.width, ext));
        }
        for &(_, cell) in &self.header_cells {
            paint_ctx.paint_child(ElementNodeId::new(cell.as_u64()), canvas);
        }
        if p.header_extent.is_some() {
            canvas.pop_clip();
        }

        let clip_rows = p.row_extent.is_some();
        for (i, row) in self.row_cells.iter().enumerate() {
            let top = self.row_tops.get(i).copied().unwrap_or(0.0);
            let h = self.row_heights.get(i).copied().unwrap_or(0.0);
            if clip_rows {
                canvas.push_clip(Offset::new(0.0, top), Size::new(p.width, h));
            }
            for &(_, cell) in row {
                paint_ctx.paint_child(ElementNodeId::new(cell.as_u64()), canvas);
            }
            if clip_rows {
                canvas.pop_clip();
            }
        }
    }
}
