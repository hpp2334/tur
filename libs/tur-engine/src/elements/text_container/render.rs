use parley::{Alignment, AlignmentOptions, FontStyle, FontWeight, GenericFamily, StyleProperty};
use tur_shared::{Color, ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::elements::text::text_layout;
use crate::elements::text_span::TextSpanElement;

use super::element::TextContainerElement;

struct SpanData {
    text: String,
    bold: bool,
    italic: bool,
    underline: bool,
    font_size: Option<f64>,
    color: Option<Color>,
}

impl ElementLayout for TextContainerElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let spans: Vec<SpanData> = children
            .iter()
            .filter_map(|&child_id| {
                cx.child_element::<TextSpanElement>(child_id).map(|span| SpanData {
                    text: span.content.clone(),
                    bold: span.bold,
                    italic: span.italic,
                    underline: span.underline,
                    font_size: span.font_size,
                    color: span.color,
                })
            })
            .collect();

        let full_text: String = spans.iter().map(|s| s.text.as_str()).collect();

        if full_text.is_empty() {
            self.cached_layout = None;
            return constraints.constrain(Size::ZERO);
        }

        let base_font_size = self.default_font_size;

        let (font_cx, text_layout_cx) = cx.text_layout_contexts();

        let mut builder = text_layout_cx.ranged_builder(font_cx, &full_text, 1.0);
        builder.push_default(StyleProperty::FontSize(base_font_size as f32));
        builder.push_default(StyleProperty::from(GenericFamily::SansSerif));

        let mut underline_ranges: Vec<(usize, usize)> = Vec::new();
        let mut byte_offset = 0usize;

        for span in &spans {
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
        layout.align(max_width, Alignment::Start, AlignmentOptions::default());

        let (layout_data, width, height) = text_layout::extract_layout_data(&mut layout, &underline_ranges);

        self.cached_layout = Some(layout_data);

        constraints.constrain(Size::new(width as f64, height as f64))
    }

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], _cx: &mut LayoutContext) {}
}

impl ElementRender for TextContainerElement {
    fn type_name(&self) -> &'static str {
        "tur_text_container"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        _children: &[ElementNodeId],
        _paint_ctx: &PaintContext,
    ) {
        if let Some(ref layout_data) = self.cached_layout {
            canvas.fill_text_layout(offset, layout_data);
        }
    }
}
