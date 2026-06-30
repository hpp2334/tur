use parley::{Alignment, AlignmentOptions, FontStyle, FontWeight, GenericFamily, StyleProperty};
use tur_shared::{Color, Constraints, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::elements::text::span_data::SpanData;
use crate::elements::text::text_layout;

use super::element::{DEFAULT_TEXT_COLOR, EditableTextElement};

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
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        // Resolve reactive props and cache `multiline` for the gesture/keyboard
        // handlers (those contexts lack store access).
        self.resolved_multiline = cx.read_val_opt(self.view.multiline.as_ref()).unwrap_or(false);
        let font_size = cx.read_val_opt(self.view.font_size.as_ref()).unwrap_or(14.0);
        let font_family = cx.read_val_opt(self.view.font_family.as_ref());
        let placeholder = cx.read_val_opt(self.view.placeholder.as_ref());
        let color = cx.read_val_opt(self.view.color.as_ref());
        let placeholder_color = cx.read_val_opt(self.view.placeholder_color.as_ref());

        // Resolve paint props here (layout holds the store); paint reads
        // `self.painting` and never touches the store.
        self.painting = super::element::EditableTextPainting {
            color,
            cursor_color: cx.read_val_opt(self.view.cursor_color.as_ref()),
        };
        let color = self.painting.color;

        let display_text = self.composition_display_text();

        // Always build a layout (even for empty text with no placeholder) so
        // the caret can be painted at byte 0 with the correct line metrics
        // when the editor is focused + empty.
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
        // An empty range (`start == end`) makes parley panic with
        // `style_run.range.start < style_run.range.end`, so guard it.
        if !full_text.is_empty() {
            builder.push(StyleProperty::Brush(brush_arr(text_color)), 0..full_text.len());
        }

        if build_from_spans {
            let mut byte_offset = 0usize;
            for span in &base_spans {
                let span_byte_len = span.text.len();
                let range = byte_offset..byte_offset + span_byte_len;
                // Skip zero-width spans: an empty style range panics in parley.
                if !range.is_empty() {
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
}
