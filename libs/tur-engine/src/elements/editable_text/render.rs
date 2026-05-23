use parley::{Alignment, AlignmentOptions, FontStyle, FontWeight, GenericFamily, StyleProperty};
use tur_shared::{Brush, Color, ComputedLayout, Constraints, Geometry, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::elements::text::paint_helpers;
use crate::elements::text::text_layout;

use super::element::EditableTextElement;

const DEFAULT_TEXT_COLOR: Color = Color::rgb(0, 0, 0);
const COMPOSITION_UNDERLINE_COLOR: Color = Color::rgb(0, 0, 0);

fn build_layout_from_spans(
    spans: &[crate::elements::text::span_data::SpanData],
    base_font_size: f64,
    constraints: &Constraints,
    cx: &mut LayoutContext,
    underline_ranges: &mut Vec<(usize, usize)>,
) -> (text_layout::TextLayoutData, f32, f32) {
    let full_text: String = spans.iter().map(|s| s.text.as_str()).collect();

    let (font_cx, text_layout_cx) = cx.text_layout_contexts();

    let mut builder = text_layout_cx.ranged_builder(font_cx, &full_text, 1.0, false);
    builder.push_default(StyleProperty::FontSize(base_font_size as f32));
    builder.push_default(StyleProperty::from(GenericFamily::SansSerif));

    let mut byte_offset = 0usize;

    for span in spans {
        let span_byte_len = span.text.len();
        let range = byte_offset..byte_offset + span_byte_len;

        if let Some(ref c) = span.color {
            builder.push(StyleProperty::Brush([c.r(), c.g(), c.b(), c.a()]), range.clone());
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

    let mut layout = builder.build(&full_text);

    let max_width = if constraints.max_width.is_finite() && constraints.max_width > 0.0 {
        Some(constraints.max_width as f32)
    } else {
        None
    };
    layout.break_all_lines(max_width);
    layout.align(Alignment::Start, AlignmentOptions::default());

    text_layout::extract_layout_data(&mut layout, underline_ranges)
}

impl ElementLayout for EditableTextElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let full_text: String = self.spans.iter().map(|s| s.text.as_str()).collect();

        if full_text.is_empty() && self.placeholder.is_none() {
            self.cached_layout = None;
            let height = self.font_size * 1.2;
            return constraints.constrain(Size::new(0.0, height));
        }

        let display_spans = if full_text.is_empty() {
            let placeholder_color = self
                .placeholder_color
                .unwrap_or(Color::rgb(153, 153, 153));
            vec![crate::elements::text::span_data::SpanData {
                text: self.placeholder.clone().unwrap_or_default(),
                bold: false,
                italic: false,
                underline: false,
                font_size: None,
                color: Some(placeholder_color),
            }]
        } else {
            self.spans.iter().map(|s| crate::elements::text::span_data::SpanData {
                text: s.text.clone(),
                bold: s.bold,
                italic: s.italic,
                underline: s.underline,
                font_size: s.font_size,
                color: s.color.or(self.color).or(Some(DEFAULT_TEXT_COLOR)),
            }).collect()
        };

        let mut underline_ranges = Vec::new();
        let (layout_data, width, height) =
            build_layout_from_spans(&display_spans, self.font_size, constraints, cx, &mut underline_ranges);

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

        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            if start != end {
                let (s, e) = if start < end { (start, end) } else { (end, start) };
                paint_helpers::paint_selection(canvas, offset, layout_data, s, e);
            }
        }

        canvas.fill_text_layout(offset, layout_data);

        if let (Some(start), Some(end)) = (self.composition_start, self.composition_end) {
            if start != end {
                paint_composition_underline(canvas, offset, layout_data, start, end);
            }
        }

        if paint_ctx.is_focused() {
            if let Some(cursor_pos) = self.cursor_position {
                let has_selection = self.selection_start.is_some()
                    && self.selection_end.is_some()
                    && self.selection_start != self.selection_end;
                if !has_selection {
                    paint_cursor(canvas, offset, layout_data, cursor_pos, self.cursor_color.or(self.color).unwrap_or(DEFAULT_TEXT_COLOR));
                }
            }
        }
    }
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
