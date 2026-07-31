use crate::core::render::brush::{Brush, Color};
use crate::core::layout::{Geometry, Offset, Size};

use crate::core::text::text_layout::TextLayoutData;

const DEFAULT_SELECTION_COLOR: Color = Color::rgba(56, 132, 255, 140);

pub fn paint_selection(
    canvas: &mut dyn crate::core::render::Canvas,
    layout_data: &TextLayoutData,
    start_byte: usize,
    end_byte: usize,
) {
    let start_line = layout_data.line_index_for_byte(start_byte);
    let end_line = layout_data.line_index_for_byte(end_byte);

    for line_idx in start_line..=end_line {
        let line_start = layout_data.line_start_byte(line_idx);
        let line_end = layout_data.line_end_byte(line_idx);

        let sel_start = start_byte.max(line_start);
        let sel_end = end_byte.min(line_end);

        if sel_start >= sel_end {
            continue;
        }

        // `cursor_x_at(N)` returns the x where the glyph at byte N starts.
        // When the selection ends exactly at a line boundary (sel_end ==
        // line_end), byte `sel_end` is the FIRST glyph of the NEXT line — its
        // x has reset to the line's left edge, so `cursor_x_at(sel_end)`
        // would return ~0 and the rect would have zero or negative width. Use
        // the right edge of the current line instead.
        let x_start = if sel_start == line_start {
            0.0
        } else {
            layout_data.cursor_x_at(sel_start)
        };
        let x_end = if sel_end == line_end {
            layout_data.line_right_x(line_idx)
        } else {
            layout_data.cursor_x_at(sel_end)
        };
        let line_info = &layout_data.line_infos[line_idx];

        // Local coordinates — the canvas transform already positions the text
        // box at its absolute origin.
        canvas.fill_geometry(
            Offset::new(x_start as f64, line_info.top as f64),
            &Geometry::Rect(Size::new(
                (x_end - x_start) as f64,
                line_info.height as f64,
            )),
            &Brush::SolidColor(DEFAULT_SELECTION_COLOR),
        );
    }
}
