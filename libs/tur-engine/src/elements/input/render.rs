use parley::{Alignment, AlignmentOptions, GenericFamily, StyleProperty};
use tur_shared::{Brush, Color, ComputedLayout, Constraints, Geometry, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::elements::text::text_layout;

use super::element::InputElement;

fn byte_to_char_offset(s: &str, byte_pos: usize) -> usize {
    s[..byte_pos].chars().count()
}

const DEFAULT_COLOR: Color = Color::rgb(255, 255, 255);
const SELECTION_COLOR: Color = Color::rgba(0, 120, 215, 77);
const COMPOSITION_UNDERLINE_COLOR: Color = Color::rgb(0, 0, 0);

fn color_to_brush(color: &Color) -> [u8; 4] {
    [color.r(), color.g(), color.b(), color.a()]
}

fn build_text_layout(
    content: &str,
    font_size: f64,
    color: &Color,
    constraints: &Constraints,
    cx: &mut LayoutContext,
) -> (text_layout::TextLayoutData, f32, f32) {
    let brush = color_to_brush(color);
    let (font_cx, text_layout_cx) = cx.text_layout_contexts();

    let mut builder = text_layout_cx.ranged_builder(font_cx, content, 1.0, false);
    builder.push_default(StyleProperty::FontSize(font_size as f32));
    builder.push_default(StyleProperty::Brush(brush));
    builder.push_default(StyleProperty::from(GenericFamily::SansSerif));

    let mut layout = builder.build(content);

    let max_width = if constraints.max_width.is_finite() && constraints.max_width > 0.0 {
        Some(constraints.max_width as f32)
    } else {
        None
    };
    layout.break_all_lines(max_width);
    layout.align(Alignment::Start, AlignmentOptions::default());

    text_layout::extract_layout_data(&mut layout, &[])
}

impl ElementLayout for InputElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let display_text = if self.content.is_empty() && self.composition_text.is_none() {
            self.placeholder.as_deref().unwrap_or("")
        } else {
            ""
        };

        let effective_text = if self.composition_text.is_some() {
            self.composition_display_text()
        } else if display_text.is_empty() {
            self.content.clone()
        } else {
            display_text.to_string()
        };

        if effective_text.is_empty() {
            self.cached_layout = None;
            let height = self.font_size * 1.2;
            return constraints.constrain(Size::new(0.0, height));
        }

        let placeholder_color = Color::rgb(153, 153, 153);
        let color = if self.content.is_empty() && self.composition_text.is_none() {
            self.placeholder_color
                .as_ref()
                .unwrap_or(&placeholder_color)
        } else {
            self.color.as_ref().unwrap_or(&DEFAULT_COLOR)
        };

        let (layout_data, width, height) =
            build_text_layout(&effective_text, self.font_size, color, constraints, cx);

        self.cached_layout = Some(layout_data);

        constraints.constrain(Size::new(width as f64, height as f64))
    }

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], _cx: &mut LayoutContext) {}
}

impl ElementRender for InputElement {
    fn type_name(&self) -> &'static str {
        "tur_input"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        _children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        if let Some(ref layout_data) = self.cached_layout {
            if self.has_selection() {
                let (start, end) = self.selection_range();
                let (sel_start, sel_end) = if let Some(ref comp) = self.composition_text {
                    let comp_len = comp.len();
                    let s = if start < self.composition_start { start } else { start + comp_len };
                    let e = if end < self.composition_start { end } else { end + comp_len };
                    (s, e)
                } else {
                    (start, end)
                };
                self.paint_selection(canvas, offset, layout_data, sel_start, sel_end);
            }

            canvas.fill_text_layout(offset, layout_data);

            if let Some(ref comp) = self.composition_text {
                let comp_len = comp.len();
                let comp_start_char = byte_to_char_offset(&self.content, self.composition_start);
                let comp_end_char = comp_start_char
                    + byte_to_char_offset(comp, comp_len);
                self.paint_composition_underline(
                    canvas,
                    offset,
                    layout_data,
                    comp_start_char,
                    comp_end_char,
                );
            }

            if paint_ctx.is_focused() && !self.has_selection() {
                let effective_cursor = if let Some(ref comp) = self.composition_text {
                    self.composition_start + comp.len()
                } else {
                    self.cursor_position
                };
                let display = self.composition_display_text();
                let char_idx = byte_to_char_offset(&display, effective_cursor);
                let (cursor_x, cursor_y) = layout_data.cursor_xy_at(char_idx);
                let line_idx = layout_data.line_index_for_char(char_idx);
                let line_height = layout_data.line_height_at(line_idx);
                let cursor_color = self
                    .cursor_color
                    .as_ref()
                    .or(self.color.as_ref())
                    .unwrap_or(&DEFAULT_COLOR);
                canvas.fill_geometry(
                    Offset::new(offset.x + cursor_x as f64, offset.y + cursor_y as f64),
                    &Geometry::Rect(Size::new(2.0, line_height as f64)),
                    &Brush::SolidColor(*cursor_color),
                );
            }
        }
    }
}

impl InputElement {
    fn paint_selection(
        &self,
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
                &Brush::SolidColor(SELECTION_COLOR),
            );
        }
    }

    fn paint_composition_underline(
        &self,
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
}
