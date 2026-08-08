use parley::{
    Alignment, AlignmentOptions, FontStyle, FontWeight, GenericFamily, Layout, StyleProperty,
};
use std::sync::Arc;

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
    base_weight: Option<f32>,
) -> (Layout<[u8; 4]>, Vec<(usize, usize)>) {
    let mut builder = text_layout_cx.ranged_builder(font_cx, text, 1.0, false);
    builder.push_default(StyleProperty::FontSize(base_font_size));
    builder.push_default(StyleProperty::from(GenericFamily::SansSerif));
    if let Some(w) = base_weight {
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(w)));
    }

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
            builder.push(
                StyleProperty::Brush([c.r(), c.g(), c.b(), c.a()]),
                range.clone(),
            );
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
            // Advance `best` past the entire grapheme cluster whose first
            // codepoint sits at `stop.byte`. parley shapes a multi-codepoint
            // grapheme (decomposed "é" = 'e' + U+0301, a ZWJ emoji sequence,
            // a regional-indicator flag) as a single glyph, but the per-glyph
            // stop records only the first codepoint's byte offset — so slicing
            // at `byte + char_len` would split the cluster and silently strip
            // the combining mark (turning "é" into "e"). Walking to the
            // cluster's end (UAX#29 grapheme boundary) keeps the prefix
            // visually intact. ASCII/Latin-1 text is unaffected (grapheme
            // length == char length).
            use unicode_segmentation::UnicodeSegmentation;
            let cluster_end = full_text[stop.byte..]
                .graphemes(true)
                .next()
                .map(|g| stop.byte + g.len())
                .unwrap_or(stop.byte);
            best = cluster_end.min(nth.end_byte);
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
        weight: None,
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
        let base_font_size = cx
            .read_val_opt(self.view.font_size.as_ref())
            .unwrap_or(14.0);
        let base_weight = cx
            .read_val_opt(self.view.font_weight.as_ref())
            .map(|w| w as f32);

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
                weight: None,
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
        let overflow = cx
            .read_val_opt(self.view.overflow.as_ref())
            .unwrap_or_default();
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
                base_weight,
            );
            layout.break_all_lines(max_width);
            layout.align(Alignment::Start, AlignmentOptions::default());

            let (layout_data, width, height) =
                text_layout::extract_layout_data(&mut layout, &underline_ranges, &full_text);
            self.cached_layout = Some(Arc::new(layout_data));
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
            base_weight,
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
            self.cached_layout = Some(Arc::new(layout_data));
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
            self.cached_layout = Some(Arc::new(probe_data));
            let width = layout.width();
            let height = layout.height();
            return constraints.constrain(Size::new(width as f64, height as f64));
        };

        let ellipsis_w = measure_ellipsis_width(font_cx, text_layout_cx, base_font_size as f32);
        let trunc_byte = compute_trunc_byte(&full_text, nth, ellipsis_w, max_width);

        let (truncated_text, truncated_spans) =
            build_truncated(&full_text, &spans, trunc_byte, default_color);

        let (mut trunc_layout, trunc_underlines) = build_parley_layout(
            font_cx,
            text_layout_cx,
            &truncated_text,
            &truncated_spans,
            base_font_size as f32,
            base_weight,
        );
        trunc_layout.break_all_lines(max_width);
        trunc_layout.align(Alignment::Start, AlignmentOptions::default());

        let (layout_data, width, height) =
            text_layout::extract_layout_data(&mut trunc_layout, &trunc_underlines, &truncated_text);
        self.cached_layout = Some(Arc::new(layout_data));
        constraints.constrain(Size::new(width as f64, height as f64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::text::text_layout::{LineGlyphStop, LineInfo};
    use unicode_segmentation::UnicodeSegmentation;

    /// Build a minimal `LineInfo` covering `[start_byte, end_byte)` with the
    /// given `(byte, x, advance)` glyph stops. `y` is unused by
    /// `compute_trunc_byte` so it's set to 0.
    fn line(start_byte: usize, end_byte: usize, stops: &[(usize, f32, f32)]) -> LineInfo {
        LineInfo {
            top: 0.0,
            height: 16.0,
            baseline: 12.0,
            start_byte,
            end_byte,
            right_x: 0.0,
            stops: stops
                .iter()
                .map(|&(byte, x, advance)| LineGlyphStop {
                    byte,
                    x,
                    y: 0.0,
                    advance,
                })
                .collect(),
        }
    }

    /// ASCII text: grapheme boundaries == char boundaries == byte
    /// boundaries, so the grapheme-aware logic must produce the same result
    /// as the original char-based advance.
    #[test]
    fn trunc_byte_ascii_no_overshoot() {
        let text = "Hello";
        // One glyph per char, advances 10/8/8/5/8 (right edges 10/18/26/31/39).
        let line = line(
            0,
            5,
            &[
                (0, 0.0, 10.0),
                (1, 10.0, 8.0),
                (2, 18.0, 8.0),
                (3, 26.0, 5.0),
                (4, 31.0, 8.0),
            ],
        );
        // Budget 30: glyphs with right + ellipsis(5) <= 30 fit.
        //   stop 0: 10+5=15 ✓ → best = 1
        //   stop 1: 18+5=23 ✓ → best = 2
        //   stop 2: 26+5=31 ✗ → break
        assert_eq!(compute_trunc_byte(text, &line, 5.0, Some(30.0)), 2);
    }

    /// The last fitting glyph is the second codepoint of a decomposed
    /// grapheme cluster (e.g. "é" = 'e' + U+0301). The char-based advance
    /// would land between 'e' and U+0301, corrupting "é" into "e" in the
    /// truncated prefix. The grapheme-aware advance must include the
    /// combining mark.
    #[test]
    fn trunc_byte_preserves_decomposed_combining_mark() {
        // "résumé" in NFD: r(1) e(1) U+0301(2) s(1) u(1) m(1) e(1) U+0301(2) = 10 bytes.
        let text = "re\u{0301}sume\u{0301}";
        assert_eq!(text.len(), 10);
        assert_eq!(text.graphemes(true).count(), 6);

        // parley would shape each "é" as a single glyph covering 3 bytes,
        // but `extract_layout_data` records the first codepoint's byte offset
        // per glyph (see the per-`line_chars.next()` walk). So stop bytes are
        // 0, 1, 4, 5, 6, 7 — NOT aligned to grapheme ends (which would be
        // 1, 4, 5, 6, 7, 10).
        let line = line(
            0,
            10,
            &[
                (0, 0.0, 10.0),  // 'r'  → right 10
                (1, 10.0, 10.0), // 'é'  → right 20 (glyph covers bytes 1..4)
                (4, 20.0, 10.0), // 's'  → right 30
                (5, 30.0, 10.0), // 'u'  → right 40
                (6, 40.0, 10.0), // 'm'  → right 50
                (7, 50.0, 10.0), // 'é'  → right 60 (glyph covers bytes 7..10)
            ],
        );

        // Budget 25: only 'r' (right 10) and 'é' (right 20) fit (with 5-wide
        // ellipsis). The char-based advance would set best=2 (between 'e' and
        // U+0301), corrupting "ré" → "re". The grapheme-aware advance sets
        // best=4 (end of the "é" cluster).
        let b = compute_trunc_byte(text, &line, 5.0, Some(25.0));
        assert_eq!(b, 4, "must end at grapheme boundary, not mid-cluster");
        // The kept prefix is exactly the source's first 4 bytes ("re" + U+0301,
        // NFD "ré") — i.e. the combining mark is preserved, not stripped.
        assert_eq!(
            &text[..b],
            "re\u{0301}",
            "kept prefix must preserve the combining mark"
        );
        assert_eq!(text[..b].graphemes(true).count(), 2);
        assert_ne!(
            b, 2,
            "regression: char-based advance would split the cluster"
        );
    }

    /// A ZWJ emoji sequence shaped as a single multi-byte glyph must remain
    /// intact — the truncation byte advances past the whole sequence, not
    /// just its first codepoint.
    #[test]
    fn trunc_byte_preserves_zwj_emoji_sequence() {
        // "👨‍👩‍👧" = man + ZWJ + woman + ZWJ + girl = 5 codepoints, 1 grapheme,
        // 11 bytes UTF-8 (each emoji codepoint = 4 bytes, ZWJ = 3 bytes:
        // 4 + 3 + 4 + 3 + 4 = 18 — let me just trust String::len).
        let emoji = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let text = format!("Hi {emoji}!");
        // 5 graphemes: "H", "i", " ", the whole family-emoji cluster, "!".
        assert_eq!(text.graphemes(true).count(), 5);

        // Manually construct stops mirroring how parley +
        // extract_layout_data would tag them: 1 stop per glyph, byte = first
        // codepoint of the cluster.
        let emoji_byte = text.find(emoji).unwrap();
        let emoji_end = emoji_byte + emoji.len();
        let exclamation_byte = emoji_end;
        let line = line(
            0,
            text.len(),
            &[
                (0, 0.0, 10.0),                // 'H'
                (1, 10.0, 8.0),                // 'i'
                (2, 18.0, 5.0),                // ' '
                (emoji_byte, 23.0, 24.0),      // 👨‍👩‍👧 (1 glyph, covers whole cluster)
                (exclamation_byte, 47.0, 6.0), // '!'
            ],
        );

        // Budget that admits the emoji (right 47) but not the trailing '!'
        // (right 53). Char-based advance would set best = emoji_byte + 4
        // (just past the first 👨 codepoint), corrupting the family emoji
        // into a lone "man". Grapheme-aware advance lands at emoji_end.
        let b = compute_trunc_byte(&text, &line, 5.0, Some(52.0));
        assert_eq!(b, emoji_end, "must end at the emoji cluster's boundary");
        assert_eq!(&text[..b], format!("Hi {emoji}"));
    }

    /// Unconstrained width (no `max_width`) short-circuits to the whole line.
    #[test]
    fn trunc_byte_unconstrained_returns_line_end() {
        let text = "Hello";
        let line = line(0, 5, &[(0, 0.0, 10.0), (1, 10.0, 8.0)]);
        assert_eq!(compute_trunc_byte(text, &line, 5.0, None), 5);
    }

    /// Budget too small for any glyph: `best` stays at the line start (the
    /// ellipsis alone will be rendered, overflowing slightly — matches
    /// Flutter's behavior of always showing the ellipsis).
    #[test]
    fn trunc_byte_tiny_budget_keeps_line_start() {
        let text = "Hello";
        let line = line(0, 5, &[(0, 0.0, 10.0), (1, 10.0, 8.0)]);
        // Budget 5, ellipsis 5: even the first glyph (right 10 + 5 = 15) doesn't fit.
        assert_eq!(compute_trunc_byte(text, &line, 5.0, Some(5.0)), 0);
    }
}
