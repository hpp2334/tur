use tur_shared::{Brush, Color, Geometry, Offset, Size};

use super::text_layout::TextLayoutData;

const DEFAULT_SELECTION_COLOR: Color = Color::rgba(0, 120, 215, 77);

pub(crate) fn paint_selection(
    canvas: &mut dyn crate::core::render::Canvas,
    offset: Offset,
    layout_data: &TextLayoutData,
    start_char: usize,
    end_char: usize,
) {
    let start_line = layout_data.line_index_for_char(start_char);
    let end_line = layout_data.line_index_for_char(end_char);

    for line_idx in start_line..=end_line {
        let line_start = layout_data.line_start_char(line_idx);
        let line_end = layout_data.line_end_char(line_idx);

        let sel_start = start_char.max(line_start);
        let sel_end = end_char.min(line_end);

        if sel_start >= sel_end {
            continue;
        }

        let x_start = layout_data.cursor_x_at(sel_start);
        let x_end = layout_data.cursor_x_at(sel_end);
        let line_info = &layout_data.line_infos[line_idx];

        canvas.fill_geometry(
            Offset::new(
                offset.x + x_start as f64,
                offset.y + line_info.top as f64,
            ),
            &Geometry::Rect(Size::new(
                (x_end - x_start) as f64,
                line_info.height as f64,
            )),
            &Brush::SolidColor(DEFAULT_SELECTION_COLOR),
        );
    }
}
