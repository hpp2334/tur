use crate::core::render::brush::{Brush, Color};
use crate::core::layout::{ComputedLayout, Geometry, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::builtin_plugins::text::elements::text_shared::paint_helpers;
use crate::core::text::text_layout;

use super::element::{CARET_BLINK_HALF_PERIOD_MS, DEFAULT_TEXT_COLOR, EditableTextElement};

const COMPOSITION_UNDERLINE_COLOR: Color = Color::rgb(0, 0, 0);

impl ElementRender for EditableTextElement {
    fn type_name(&self) -> &'static str {
        "tur_editable_text"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        _children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        let color = self.painting.color;
        let cursor_color = self.painting.cursor_color;
        let obscured = self.resolved_obscured;

        let c = self.controller();
        let cursor_pos = c.cursor_position();
        let sel_anchor = c.selection_anchor();
        let sel_end = c.selection_end();
        let has_selection = c.has_selection();
        let composing_text = c.composing_text().cloned();
        let composing_start = c.composing_start();
        let text_is_empty = c.text().is_empty();
        drop(c);

        let is_focused = paint_ctx.is_focused();

        let Some(layout_data) = self.cached_layout.as_ref() else {
            return;
        };

        if is_focused && has_selection {
            let (a, b) = if sel_anchor < sel_end {
                (sel_anchor, sel_end)
            } else {
                (sel_end, sel_anchor)
            };
            paint_helpers::paint_selection(canvas, offset, layout_data, a, b);
        }

        // Hide the placeholder text when the input is focused and empty —
        // matches browser/Flutter input convention. The box keeps its
        // size (the placeholder still drives layout) so the cursor stays
        // anchored at the right position.
        let suppress_text_fill = is_focused && text_is_empty;
        if !suppress_text_fill {
            canvas.fill_text_layout(offset, layout_data);
        }

        // The composition underline's byte math targets the composition-
        // substituted display string; under password mode the layout is built
        // from the masked string (and composition is rendered as bullets), so
        // skip it.
        if !obscured
            && let Some(ref comp) = composing_text
        {
            let comp_start_byte = composing_start;
            let comp_end_byte = composing_start + comp.len();
            if comp_start_byte != comp_end_byte {
                paint_composition_underline(canvas, offset, layout_data, comp_start_byte, comp_end_byte);
            }
        }

        if is_focused && !has_selection {
            // Blink the caret at a fixed half-cycle. Visible on even
            // half-cycles. The blink loop (spawned in `on_focus_changed`)
            // sleeps for `CARET_BLINK_HALF_PERIOD_MS` then calls
            // `request_paint`, so the engine wakes at each toggle boundary.
            let now_ms = paint_ctx.now().as_millis() as u64;
            let blink_visible = (now_ms / CARET_BLINK_HALF_PERIOD_MS).is_multiple_of(2);
            if blink_visible {
                paint_cursor(
                    canvas,
                    offset,
                    layout_data,
                    cursor_pos,
                    cursor_color.or(color).unwrap_or(DEFAULT_TEXT_COLOR),
                );
            }
        }
    }
}

fn paint_composition_underline(
    canvas: &mut dyn Canvas,
    offset: Offset,
    layout_data: &text_layout::TextLayoutData,
    start_byte: usize,
    end_byte: usize,
) {
    let start_line = layout_data.line_index_for_byte(start_byte);
    let end_line = layout_data.line_index_for_byte(end_byte);

    for line_idx in start_line..=end_line {
        let line_start = layout_data.line_start_byte(line_idx);
        let line_end = layout_data.line_end_byte(line_idx);

        let ul_start = start_byte.max(line_start);
        let ul_end = end_byte.min(line_end);

        if ul_start >= ul_end {
            continue;
        }

        let x_start = if ul_start == line_start {
            0.0
        } else {
            layout_data.cursor_x_at(ul_start)
        };
        let x_end = if ul_end == line_end {
            layout_data.line_right_x(line_idx)
        } else {
            layout_data.cursor_x_at(ul_end)
        };
        let line_info = &layout_data.line_infos[line_idx];

        let underline_y = offset.y + line_info.top as f64 + line_info.height as f64 - 2.0;

        canvas.fill_geometry(
            Offset::new(offset.x + x_start as f64, underline_y),
            &Geometry::Rect(Size::new((x_end - x_start) as f64, 2.0)),
            &Brush::SolidColor(COMPOSITION_UNDERLINE_COLOR),
        );
    }
}

fn paint_cursor(
    canvas: &mut dyn Canvas,
    offset: Offset,
    layout_data: &text_layout::TextLayoutData,
    cursor_byte: usize,
    cursor_color: Color,
) {
    let (cursor_x, _) = layout_data.cursor_xy_at(cursor_byte);
    let line_idx = layout_data.line_index_for_byte(cursor_byte);
    let line_info = &layout_data.line_infos[line_idx];

    canvas.fill_geometry(
        Offset::new(offset.x + cursor_x as f64, offset.y + line_info.top as f64),
        &Geometry::Rect(Size::new(2.0, line_info.height as f64)),
        &Brush::SolidColor(cursor_color),
    );
}
