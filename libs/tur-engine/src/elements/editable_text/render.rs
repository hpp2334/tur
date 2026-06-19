use parley::{Alignment, AlignmentOptions, FontStyle, FontWeight, GenericFamily, StyleProperty};
use tur_shared::{Brush, Color, ComputedLayout, Constraints, Geometry, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::elements::text::paint_helpers;
use crate::elements::text::span_data::SpanData;
use crate::elements::text::text_layout;

use super::element::EditableTextElement;

const DEFAULT_TEXT_COLOR: Color = Color::rgb(0, 0, 0);
const COMPOSITION_UNDERLINE_COLOR: Color = Color::rgb(0, 0, 0);

/// Map a `fontFamily` string to a parley generic family. Accepts the common
/// Flutter-style names; falls back to sans-serif. "monospace" is the value
/// used by the code editor.
fn generic_family_for(font_family: Option<&str>) -> GenericFamily {
    match font_family {
        Some(f) if f.eq_ignore_ascii_case("monospace") => GenericFamily::Monospace,
        Some(f) if f.eq_ignore_ascii_case("serif") => GenericFamily::Serif,
        _ => GenericFamily::SansSerif,
    }
}

fn brush_arr(c: Color) -> [u8; 4] {
    [c.r(), c.g(), c.b(), c.a()]
}

impl ElementLayout for EditableTextElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        // Resolve reactive props and cache `multiline` for the gesture/keyboard
        // handlers (those contexts lack store access).
        self.resolved_multiline = cx.read_val_opt(self.component.multiline.as_ref()).unwrap_or(false);
        let font_size = cx.read_val_opt(self.component.font_size.as_ref()).unwrap_or(14.0);
        let font_family = cx.read_val_opt(self.component.font_family.as_ref());
        let placeholder = cx.read_val_opt(self.component.placeholder.as_ref());
        let color = cx.read_val_opt(self.component.color.as_ref());
        let placeholder_color = cx.read_val_opt(self.component.placeholder_color.as_ref());

        let display_text = self.composition_display_text();

        if display_text.is_empty() && placeholder.is_none() {
            self.cached_layout = None;
            let height = font_size * 1.2;
            return constraints.constrain(Size::new(0.0, height));
        }

        let (font_cx, text_layout_cx) = cx.text_layout_contexts();

        // Flutter-aligned: render the controller's span tree (so per-range
        // colors from syntax highlighting are visible). We fall back to a
        // flat single-color layout during IME composition (the composition
        // text is substituted into the display string, so byte offsets from
        // the base spans no longer line up) or when the controller has no
        // spans yet (placeholder display).
        let (is_composing, base_spans): (bool, Vec<SpanData>) = {
            let c = self.controller();
            (c.is_composing(), c.spans().to_vec())
        };

        let text_color = if display_text.is_empty() {
            placeholder_color.unwrap_or(Color::rgb(153, 153, 153))
        } else {
            color.unwrap_or(DEFAULT_TEXT_COLOR)
        };

        let mut underline_ranges: Vec<(usize, usize)> = Vec::new();

        // Flutter-aligned: render the controller's span tree (so per-range
        // colors from syntax highlighting are visible). We fall back to a flat
        // single-color layout during IME composition (the composition text is
        // substituted into the display string, so byte offsets from the base
        // spans no longer line up) or when the controller has no spans yet.
        let build_from_spans = !is_composing && !base_spans.is_empty() && !display_text.is_empty();

        let full_text: String = if build_from_spans {
            base_spans.iter().map(|s| s.text.as_str()).collect()
        } else if display_text.is_empty() {
            placeholder.as_deref().unwrap_or("").to_string()
        } else {
            display_text.clone()
        };

        let mut builder = text_layout_cx.ranged_builder(font_cx, &full_text, 1.0, false);
        builder.push_default(StyleProperty::FontSize(font_size as f32));
        builder.push_default(StyleProperty::from(generic_family_for(font_family.as_deref())));
        // Base color over the whole text; per-span colors override below.
        builder.push(StyleProperty::Brush(brush_arr(text_color)), 0..full_text.len());

        if build_from_spans {
            let mut byte_offset = 0usize;
            for span in &base_spans {
                let span_byte_len = span.text.len();
                let range = byte_offset..byte_offset + span_byte_len;
                if let Some(c) = &span.color {
                    builder.push(StyleProperty::Brush(brush_arr(*c)), range.clone());
                }
                if span.bold {
                    builder.push(StyleProperty::FontWeight(FontWeight::BOLD), range.clone());
                }
                if span.italic {
                    builder.push(StyleProperty::FontStyle(FontStyle::Italic), range.clone());
                }
                if let Some(fs) = span.font_size {
                    builder.push(StyleProperty::FontSize(fs as f32), range.clone());
                }
                if span.underline {
                    underline_ranges.push((byte_offset, byte_offset + span_byte_len));
                }
                byte_offset += span_byte_len;
            }
        }

        let mut layout = builder.build(&full_text);

        let max_width = if constraints.max_width.is_finite() && constraints.max_width > 0.0 {
            Some(constraints.max_width as f32)
        } else {
            None
        };
        layout.break_all_lines(max_width);
        layout.align(Alignment::Start, AlignmentOptions::default());

        let (layout_data, width, height) =
            text_layout::extract_layout_data(&mut layout, &underline_ranges, &full_text);

        self.cached_layout = Some(layout_data);

        constraints.constrain(Size::new(width as f64, height as f64))
    }

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], _cx: &mut LayoutContext) {}
}

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
        let Some(ref layout_data) = self.cached_layout else {
            return;
        };

        let color = paint_ctx.read_val_opt(self.component.color.as_ref());
        let cursor_color = paint_ctx.read_val_opt(self.component.cursor_color.as_ref());

        let c = self.controller();
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
            paint_helpers::paint_selection(canvas, offset, layout_data, a, b);
        }

        canvas.fill_text_layout(offset, layout_data);

        if let Some(ref comp) = composing_text {
            let comp_start_byte = composing_start;
            let comp_end_byte = composing_start + comp.len();
            if comp_start_byte != comp_end_byte {
                paint_composition_underline(canvas, offset, layout_data, comp_start_byte, comp_end_byte);
            }
        }

        if paint_ctx.is_focused() && !has_selection {
            paint_cursor(
                canvas, offset, layout_data, cursor_pos,
                cursor_color.or(color).unwrap_or(DEFAULT_TEXT_COLOR),
            );
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
