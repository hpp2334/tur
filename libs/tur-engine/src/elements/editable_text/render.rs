use parley::{Alignment, AlignmentOptions, GenericFamily, StyleProperty};
use tur_shared::{Brush, Color, ComputedLayout, Constraints, Geometry, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::elements::text::paint_helpers;
use crate::elements::text::text_layout;

use super::element::EditableText;

const DEFAULT_TEXT_COLOR: Color = Color::rgb(0, 0, 0);
const COMPOSITION_UNDERLINE_COLOR: Color = Color::rgb(0, 0, 0);

impl ElementLayout for EditableText {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        // Resolve reactive props and cache `multiline` for the gesture/keyboard
        // handlers (those contexts lack store access).
        self.resolved_multiline = cx.read_val_opt(self.spec.multiline.as_ref()).unwrap_or(false);
        let font_size = cx.read_val_opt(self.spec.font_size.as_ref()).unwrap_or(14.0);
        let placeholder = cx.read_val_opt(self.spec.placeholder.as_ref());
        let color = cx.read_val_opt(self.spec.color.as_ref());
        let placeholder_color = cx.read_val_opt(self.spec.placeholder_color.as_ref());

        let display_text = self.composition_display_text();

        if display_text.is_empty() && placeholder.is_none() {
            self.cached_layout = None;
            let height = font_size * 1.2;
            return constraints.constrain(Size::new(0.0, height));
        }

        let (font_cx, text_layout_cx) = cx.text_layout_contexts();

        let text = if display_text.is_empty() {
            placeholder.as_deref().unwrap_or("")
        } else {
            &display_text
        };

        let text_color = if display_text.is_empty() {
            placeholder_color.unwrap_or(Color::rgb(153, 153, 153))
        } else {
            color.unwrap_or(DEFAULT_TEXT_COLOR)
        };

        let mut builder = text_layout_cx.ranged_builder(font_cx, text, 1.0, false);
        builder.push_default(StyleProperty::FontSize(font_size as f32));
        builder.push_default(StyleProperty::from(GenericFamily::SansSerif));
        builder.push(StyleProperty::Brush([text_color.r(), text_color.g(), text_color.b(), text_color.a()]), 0..text.len());

        let mut layout = builder.build(text);

        let max_width = if constraints.max_width.is_finite() && constraints.max_width > 0.0 {
            Some(constraints.max_width as f32)
        } else {
            None
        };
        layout.break_all_lines(max_width);
        layout.align(Alignment::Start, AlignmentOptions::default());

        let underline_ranges = Vec::new();
        let (layout_data, width, height) = text_layout::extract_layout_data(&mut layout, &underline_ranges);

        self.cached_layout = Some(layout_data);

        constraints.constrain(Size::new(width as f64, height as f64))
    }

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], _cx: &mut LayoutContext) {}
}

impl ElementRender for EditableText {
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
        let Some(ref layout_data) = self.cached_layout else {
            return;
        };

        let color = paint_ctx.read_val_opt(self.spec.color.as_ref());
        let cursor_color = paint_ctx.read_val_opt(self.spec.cursor_color.as_ref());

        let c = self.controller();
        let full = c.text();
        let cursor_pos = c.cursor_position();
        let sel_anchor = c.selection_anchor();
        let sel_end = c.selection_end();
        let has_selection = c.has_selection();
        let composing_text = c.composing_text().cloned();
        let composing_start = c.composing_start();
        drop(c);

        if has_selection {
            let (a, b) = if sel_anchor < sel_end {
                (sel_anchor, sel_end)
            } else {
                (sel_end, sel_anchor)
            };
            let a_char = byte_to_char_offset(&full, a);
            let b_char = byte_to_char_offset(&full, b);
            paint_helpers::paint_selection(canvas, offset, layout_data, a_char, b_char);
        }

        canvas.fill_text_layout(offset, layout_data);

        if let Some(ref comp) = composing_text {
            let comp_start_char = byte_to_char_offset(&full, composing_start);
            let comp_end_char = comp_start_char + comp.chars().count();
            if comp_start_char != comp_end_char {
                paint_composition_underline(canvas, offset, layout_data, comp_start_char, comp_end_char);
            }
        }

        if paint_ctx.is_focused() && !has_selection {
            let cursor_char = byte_to_char_offset(&full, cursor_pos);
            paint_cursor(
                canvas, offset, layout_data, cursor_char,
                cursor_color.or(color).unwrap_or(DEFAULT_TEXT_COLOR),
            );
        }
    }
}

fn byte_to_char_offset(s: &str, byte_pos: usize) -> usize {
    s[..byte_pos.min(s.len())].chars().count()
}

fn paint_composition_underline(
    canvas: &mut dyn Canvas,
    offset: Offset,
    layout_data: &text_layout::TextLayoutData,
    start_char: usize,
    end_char: usize,
) {
    let start_line = layout_data.line_index_for_char(start_char);
    let end_line = layout_data.line_index_for_char(end_char);

    for line_idx in start_line..=end_line {
        let line_start = layout_data.line_start_char(line_idx);
        let line_end = layout_data.line_end_char(line_idx);

        let ul_start = start_char.max(line_start);
        let ul_end = end_char.min(line_end);

        if ul_start >= ul_end {
            continue;
        }

        let x_start = layout_data.cursor_x_at(ul_start);
        let x_end = layout_data.cursor_x_at(ul_end);
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
    cursor_pos: usize,
    cursor_color: Color,
) {
    let (cursor_x, _) = layout_data.cursor_xy_at(cursor_pos);
    let line_idx = layout_data.line_index_for_char(cursor_pos);
    let line_info = &layout_data.line_infos[line_idx];

    canvas.fill_geometry(
        Offset::new(offset.x + cursor_x as f64, offset.y + line_info.top as f64),
        &Geometry::Rect(Size::new(2.0, line_info.height as f64)),
        &Brush::SolidColor(cursor_color),
    );
}
