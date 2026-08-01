use crate::core::layout::{Constraints, Size};
use crate::core::render::brush::Color;
use parley::{Alignment, AlignmentOptions, FontStyle, FontWeight, GenericFamily, StyleProperty};

use crate::builtin_plugins::text::elements::text_shared::span_data::SpanData;
use crate::builtin_plugins::text::text_layout;
use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

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
        self.resolved_multiline = cx
            .read_val_opt(self.view.multiline.as_ref())
            .unwrap_or(false);
        let font_size = cx
            .read_val_opt(self.view.font_size.as_ref())
            .unwrap_or(14.0);
        let font_family = cx.read_val_opt(self.view.font_family.as_ref());
        let font_weight = cx
            .read_val_opt(self.view.font_weight.as_ref())
            .map(|w| w as f32);
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

        // Resolve password-mode props (refreshed each layout so the gesture/
        // keyboard/IME/render handlers — which lack store access — read the
        // latest values). When obscured, layout builds from a masked display
        // string (each char → obscuringCharacter) and then remaps the
        // layout's byte offsets back into the controller's value-byte space,
        // so all cursor/selection/caret/click math works unchanged.
        self.resolved_obscured = cx
            .read_val_opt(self.view.obscure_text.as_ref())
            .unwrap_or(false);
        self.resolved_obscuring_char = cx
            .read_val_opt(self.view.obscuring_character.as_ref())
            .and_then(|s| s.chars().next())
            .unwrap_or('\u{2022}');
        let obscured = self.resolved_obscured;

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
        // spans no longer line up), when password mode is active (the masked
        // display string shares no byte offsets with the span tree), or when
        // the controller has no spans yet (placeholder display).
        let build_from_spans =
            !obscured && !is_composing && !base_spans.is_empty() && !display_text.is_empty();

        // When obscured, render the masked display string (each char of the
        // value — and any in-progress composition — replaced by the obscuring
        // char) and remember the char→value-byte map so we can remap the
        // layout below. An empty masked value falls through to the placeholder
        // path so the hint still shows (unmasked).
        let masked: Option<(String, Vec<usize>, usize)> =
            if obscured { self.build_masked() } else { None };
        // Active mask: a non-empty masked value to render.
        let active_mask: Option<&(String, Vec<usize>, usize)> = match masked.as_ref() {
            Some((m, _, _)) if !m.is_empty() => masked.as_ref(),
            _ => None,
        };
        let remap: Option<(Vec<usize>, usize)> = active_mask.map(|(_, map, ml)| (map.clone(), *ml));

        let full_text: String = if build_from_spans {
            base_spans.iter().map(|s| s.text.as_str()).collect()
        } else if let Some((m, _, _)) = active_mask {
            m.clone()
        } else if display_text.is_empty() {
            placeholder.as_deref().unwrap_or("").to_string()
        } else {
            display_text.clone()
        };

        let mut builder = text_layout_cx.ranged_builder(font_cx, &full_text, 1.0, false);
        builder.push_default(StyleProperty::FontSize(font_size as f32));
        builder.push_default(StyleProperty::from(generic_family_for(
            font_family.as_deref(),
        )));
        if let Some(w) = font_weight {
            builder.push_default(StyleProperty::FontWeight(FontWeight::new(w)));
        }
        // Base color over the whole text; per-span colors override below.
        // An empty range (`start == end`) makes parley panic with
        // `style_run.range.start < style_run.range.end`, so guard it.
        if !full_text.is_empty() {
            builder.push(
                StyleProperty::Brush(brush_arr(text_color)),
                0..full_text.len(),
            );
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
                    if let Some(w) = span.weight {
                        builder.push(
                            StyleProperty::FontWeight(FontWeight::new(w as f32)),
                            range.clone(),
                        );
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

        let (mut layout_data, width, height) =
            text_layout::extract_layout_data(&mut layout, &underline_ranges, &full_text);

        // Remap the masked layout's byte offsets back into the controller's
        // value-byte space so cursor/selection/caret/click math (all of which
        // operate in value bytes) works unchanged.
        if let Some((map, ml)) = remap {
            remap_layout_bytes(&mut layout_data, &map, ml);
        }

        self.cached_layout = Some(layout_data);

        constraints.constrain(Size::new(width as f64, height as f64))
    }
}

/// Remap a masked-display layout's byte offsets back into the controller's
/// value-byte space. Every glyph stop / line boundary sits at a display-char
/// boundary (each mask char is `mask_len` bytes), so display byte `b` maps to
/// `map[b / mask_len]`. `map` holds one entry per display char plus a final
/// entry for the end-of-string position.
fn remap_layout_bytes(
    layout_data: &mut crate::core::text::text_layout::TextLayoutData,
    map: &[usize],
    mask_len: usize,
) {
    let len = map.len();
    if len == 0 || mask_len == 0 {
        return;
    }
    let at = |display_byte: usize| -> usize {
        let idx = (display_byte / mask_len).min(len - 1);
        map[idx]
    };
    for line in &mut layout_data.line_infos {
        line.start_byte = at(line.start_byte);
        line.end_byte = at(line.end_byte);
        for stop in &mut line.stops {
            stop.byte = at(stop.byte);
        }
    }
}
