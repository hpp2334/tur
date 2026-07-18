use parley::{Alignment, AlignmentOptions, FontStyle, FontWeight, GenericFamily, StyleProperty};
use tur_engine::core::layout::{Constraints, Size};

use tur_engine::core::element::ElementNodeId;
use tur_engine::core::layout::{ElementLayout, LayoutContext};
use crate::elements::text_shared::span_data::SpanData;
use crate::text_layout;

use super::element::TextElement;

impl ElementLayout for TextElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let base_font_size = cx.read_val_opt(self.view.font_size.as_ref()).unwrap_or(14.0);

        // Resolve the spans to lay out. If the spec carries explicit spans,
        // use them; otherwise build a single anonymous span from the `text`
        // prop (the common "plain TextElement" case).
        let spans: Vec<SpanData> = if let Some(s) = self.view.spans.as_ref() {
            s.clone()
        } else {
            let text = cx.read_val_opt(self.view.text.as_ref()).unwrap_or_default();
            let color = cx.read_val_opt(self.view.color.as_ref());
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

            // Skip zero-width spans: an empty style range panics in parley
            // (`style_run.range.start < style_run.range.end`).
            if !range.is_empty() {
                // Fall back to opaque black when no color is set — without an
                // explicit brush, parley/vello render text invisibly.
                let c = span.color.unwrap_or(tur_engine::core::render::Color::rgb(0, 0, 0));
                builder.push(StyleProperty::Brush([c.r(), c.g(), c.b(), c.a()]), range.clone());
                if span.bold {
                    builder.push(StyleProperty::FontWeight(FontWeight::BOLD), range.clone());
                }
                if span.italic {
                    builder.push(StyleProperty::FontStyle(FontStyle::Italic), range.clone());
                }
                if let Some(fs) = span.font_size {
                    builder.push(StyleProperty::FontSize(fs as f32), range.clone());
                }
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
}
