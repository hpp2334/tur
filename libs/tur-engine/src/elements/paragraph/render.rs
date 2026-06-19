use parley::{Alignment, AlignmentOptions, FontStyle, FontWeight, GenericFamily, StyleProperty};
use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::elements::text::paint_helpers;
use crate::elements::text::span_data::SpanData;
use crate::elements::text::text_layout;

use super::element::TextElement;

impl ElementLayout for TextElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let base_font_size = cx.read_val_opt(self.component.font_size.as_ref()).unwrap_or(14.0);

        // Resolve the spans to lay out. If the spec carries explicit spans,
        // use them; otherwise build a single anonymous span from the `text`
        // prop (the common "plain TextElement" case).
        let spans: Vec<SpanData> = if let Some(s) = self.component.spans.as_ref() {
            s.clone()
        } else {
            let text = cx.read_val_opt(self.component.text.as_ref()).unwrap_or_default();
            let color = cx.read_val_opt(self.component.color.as_ref());
            vec![SpanData {
                text,
                bold: false,
                italic: false,
                underline: false,
                font_size: None,
                color,
            }]
        };

        // Cache the resolved spans so test code can read the current text
        // via `TextElement::spans()` without re-resolving.
        self.cached_spans = spans.clone();

        let full_text: String = spans.iter().map(|s| s.text.as_str()).collect();

        if full_text.is_empty() {
            self.cached_layout = None;
            return constraints.constrain(Size::ZERO);
        }

        let (font_cx, text_layout_cx) = cx.text_layout_contexts();

        let mut builder = text_layout_cx.ranged_builder(font_cx, &full_text, 1.0, false);
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
        layout.align(Alignment::Start, AlignmentOptions::default());

        let (layout_data, width, height) =
            text_layout::extract_layout_data(&mut layout, &underline_ranges, &full_text);

        self.cached_layout = Some(layout_data);

        constraints.constrain(Size::new(width as f64, height as f64))
    }

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], _cx: &mut LayoutContext) {}
}

impl ElementRender for TextElement {
    fn type_name(&self) -> &'static str {
        "tur_paragraph"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        _children: &[ElementNodeId],
        _paint_ctx: &PaintContext,
    ) {
        let Some(ref layout_data) = self.cached_layout else {
            return;
        };

        if self.selection_anchor != self.selection_end {
            let (s, e) = if self.selection_anchor < self.selection_end {
                (self.selection_anchor, self.selection_end)
            } else {
                (self.selection_end, self.selection_anchor)
            };
            paint_helpers::paint_selection(canvas, offset, layout_data, s, e);
        }

        canvas.fill_text_layout(offset, layout_data);
    }
}
