use parley::{
    Alignment, AlignmentOptions, FontStyle, FontWeight, GenericFamily, Layout, StyleProperty,
};

use crate::core::element::ElementNodeId;
use crate::core::layout::{Constraints, ElementLayout, LayoutContext, Size};
use crate::core::render::brush::Color;
use crate::core::text::text_layout::LineInfo;

use crate::builtin_plugins::text::elements::text_shared::span_data::SpanData;
use crate::builtin_plugins::text::text_layout;

use super::element::{TextElement, TextOverflow};

/// Build an unbroken parley layout for `text` styled by `spans`.
///
/// Shared by the main layout path and the ellipsis-truncation rebuild. Returns
/// the built `Layout` (caller still needs to `break_all_lines` / `break_lines`
/// and `align`) plus the per-span underline ranges.
fn build_parley_layout(
    font_cx: &mut parley::FontContext,
    text_layout_cx: &mut parley::LayoutContext<[u8; 4]>,
    text: &str,
    spans: &[SpanData],
    base_font_size: f32,
) -> (Layout<[u8; 4]>, Vec<(usize, usize)>) {
    let mut builder = text_layout_cx.ranged_builder(font_cx, text, 1.0, false);
    builder.push_default(StyleProperty::FontSize(base_font_size));
    builder.push_default(StyleProperty::from(GenericFamily::SansSerif));

    let mut underline_ranges: Vec<(usize, usize)> = Vec::new();
    let mut byte_offset = 0usize;

    for span in spans {
        let span_byte_len = span.text.len();
        let range = byte_offset..byte_offset + span_byte_len;

        // Skip zero-width spans: an empty style range panics in parley
        // (`style_run.range.start < style_run.range.end`).
        if !range.is_empty() {
            // Fall back to opaque black when no color is set — without an
            // explicit brush, parley/vello render text invisibly.
            let c = span.color.unwrap_or(Color::rgb(0, 0, 0));
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

    (builder.build(text), underline_ranges)
}

/// Measure the advance width of a single `…` glyph at `font_size`.
///
/// Used to budget the last visible line so that `prefix + …` fits within the
/// line's `max_width`.
fn measure_ellipsis_width(
    font_cx: &mut parley::FontContext,
    text_layout_cx: &mut parley::LayoutContext<[u8; 4]>,
    font_size: f32,
) -> f32 {
    let mut builder = text_layout_cx.ranged_builder(font_cx, "…", 1.0, false);
    builder.push_default(StyleProperty::FontSize(font_size));
    builder.push_default(StyleProperty::from(GenericFamily::SansSerif));
    let mut layout: Layout<[u8; 4]> = builder.build("…");
    layout.break_all_lines(None);
    layout.width()
}

/// Walk the Nth line's glyph stops and return the longest char-safe byte offset
/// `b` (relative to `full_text`) such that the prefix
/// `full_text[nth.start_byte..b]` plus an ellipsis fits within `max_width`.
///
/// When `max_width` is `None` (unconstrained), returns `nth.end_byte` — the
/// whole line plus `…` (no trimming needed because there's no width budget
/// to overflow).
fn compute_trunc_byte(
    full_text: &str,
    nth: &LineInfo,
    ellipsis_width: f32,
    max_width: Option<f32>,
) -> usize {
    let Some(max) = max_width else {
        return nth.end_byte;
    };
    let mut best = nth.start_byte;
    for stop in &nth.stops {
        let right = stop.x + stop.advance;
        if right + ellipsis_width <= max {
            // The char at `stop.byte` fits; advance `best` past it (char-safe).
            let char_len = full_text[stop.byte..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            best = stop.byte + char_len;
        } else {
            break;
        }
    }
    best
}

/// Build the truncated `<full_text[..trunc_byte]>…` plus the matching span
/// list. Original spans are kept up to `trunc_byte` (a straddling span is
/// prefix-truncated on a char boundary); a final `…` span with `default_color`
/// is appended so the ellipsis glyph is visible (parley renders brush-less
/// ranges invisibly).
fn build_truncated(
    full_text: &str,
    spans: &[SpanData],
    trunc_byte: usize,
    default_color: Color,
) -> (String, Vec<SpanData>) {
    let mut text = String::with_capacity(trunc_byte + 3);
    text.push_str(&full_text[..trunc_byte]);
    text.push('…');

    let mut out: Vec<SpanData> = Vec::with_capacity(spans.len() + 1);
    let mut off = 0usize;
    for span in spans {
        if off >= trunc_byte {
            break;
        }
        let span_len = span.text.len();
        if off + span_len <= trunc_byte {
            out.push(span.clone());
        } else {
            let keep_bytes = trunc_byte - off;
            let mut truncated = span.clone();
            // `trunc_byte` is a char boundary in `full_text`; the
            // corresponding offset in this span's text is too.
            truncated.text = span.text[..keep_bytes].to_string();
            out.push(truncated);
        }
        off += span_len;
    }
    out.push(SpanData {
        text: "…".to_string(),
        bold: false,
        italic: false,
        underline: false,
        font_size: None,
        color: Some(default_color),
    });

    (text, out)
}

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

        let max_width = if constraints.max_width.is_finite() && constraints.max_width > 0.0 {
            Some(constraints.max_width as f32)
        } else {
            None
        };

        let max_lines = cx.read_val_opt(self.view.max_lines.as_ref());
        let overflow = cx.read_val_opt(self.view.overflow.as_ref()).unwrap_or_default();
        // Resolve the default color once, up-front: `cx` is mutably borrowed by
        // `text_layout_contexts()` for the rest of the function, so we can't
        // touch it again after that point.
        let default_color = cx
            .read_val_opt(self.view.color.as_ref())
            .unwrap_or_else(|| Color::rgb(0, 0, 0));

        // Truncation applies only when an explicit `maxLines > 0` is set AND
        // the overflow mode is not `Visible` (which ignores `maxLines`,
        // matching Flutter).
        let truncate = matches!(max_lines, Some(n) if n > 0) && overflow != TextOverflow::Visible;

        let (font_cx, text_layout_cx) = cx.text_layout_contexts();

        if !truncate {
            let (mut layout, underline_ranges) = build_parley_layout(
                font_cx,
                text_layout_cx,
                &full_text,
                &spans,
                base_font_size as f32,
            );
            layout.break_all_lines(max_width);
            layout.align(Alignment::Start, AlignmentOptions::default());

            let (layout_data, width, height) =
                text_layout::extract_layout_data(&mut layout, &underline_ranges, &full_text);
            self.cached_layout = Some(layout_data);
            return constraints.constrain(Size::new(width as f64, height as f64));
        }

        let n = max_lines.unwrap() as usize;

        // Cap at N lines via the incremental breaker.
        let (mut layout, underline_ranges) = build_parley_layout(
            font_cx,
            text_layout_cx,
            &full_text,
            &spans,
            base_font_size as f32,
        );
        let leftover = {
            let mut breaker = layout.break_lines();
            if let Some(max) = max_width {
                // parley's `break_next` asserts `line_max_advance` ≈
                // `layout_max_advance`; mirror what `break_remaining` does.
                breaker.state_mut().set_layout_max_advance(max);
                breaker.state_mut().set_line_max_advance(max);
            }
            for _ in 0..n {
                if breaker.break_next().is_none() {
                    break;
                }
            }
            // `Drop` finalizes the layout with at most N committed lines.
            !breaker.is_done()
        };
        layout.align(Alignment::Start, AlignmentOptions::default());

        // No leftover → no ellipsis needed (Clip or Ellipsis produce the same
        // result when the text already fits in N lines).
        let need_ellipsis = overflow == TextOverflow::Ellipsis && leftover;
        if !need_ellipsis {
            let (layout_data, width, height) =
                text_layout::extract_layout_data(&mut layout, &underline_ranges, &full_text);
            self.cached_layout = Some(layout_data);
            return constraints.constrain(Size::new(width as f64, height as f64));
        }

        // Ellipsis mode with leftover: probe the capped layout to read the
        // Nth line's glyph stops, then rebuild a fresh layout for
        // `<prefix>…`.
        let (probe_data, _, _) =
            text_layout::extract_layout_data(&mut layout, &underline_ranges, &full_text);

        let Some(nth) = probe_data.line_infos.get(n - 1) else {
            // Fewer than N lines despite `leftover` (defensive — parley's
            // breaker shouldn't allow this). Fall back to the capped layout.
            self.cached_layout = Some(probe_data);
            let width = layout.width();
            let height = layout.height();
            return constraints.constrain(Size::new(width as f64, height as f64));
        };

        let ellipsis_w =
            measure_ellipsis_width(font_cx, text_layout_cx, base_font_size as f32);
        let trunc_byte = compute_trunc_byte(&full_text, nth, ellipsis_w, max_width);

        let (truncated_text, truncated_spans) =
            build_truncated(&full_text, &spans, trunc_byte, default_color);

        let (mut trunc_layout, trunc_underlines) = build_parley_layout(
            font_cx,
            text_layout_cx,
            &truncated_text,
            &truncated_spans,
            base_font_size as f32,
        );
        trunc_layout.break_all_lines(max_width);
        trunc_layout.align(Alignment::Start, AlignmentOptions::default());

        let (layout_data, width, height) = text_layout::extract_layout_data(
            &mut trunc_layout,
            &trunc_underlines,
            &truncated_text,
        );
        self.cached_layout = Some(layout_data);
        constraints.constrain(Size::new(width as f64, height as f64))
    }
}
