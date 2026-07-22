//! Text layout extraction: walks a parley `Layout` and produces a
//! `crate::core::text::TextLayoutData` (pure-data struct the engine's
//! `Canvas::fill_text_layout` consumes). The data type lives in the engine
//! as part of the paint contract; this function (the parley consumer) lives
//! here in `tur-text`.

use parley::layout::PositionedLayoutItem;

use crate::core::text::text_layout::{
    LineGlyphStop, LineInfo, TextGlyph, TextLayoutData, TextRunData,
};

/// Walk `layout` and produce `(TextLayoutData, width, height)`.
///
/// Each glyph is tagged with its source-string byte offset so the layout's
/// geometry (x/y) stays aligned with `TextEditingController`'s byte-based
/// cursor/selection offsets. Newlines (`\n`) consume a byte but produce no
/// glyph.
pub fn extract_layout_data(
    layout: &mut parley::Layout<[u8; 4]>,
    underline_ranges: &[(usize, usize)],
    full_text: &str,
) -> (TextLayoutData, f32, f32) {
    let width = layout.width();
    let height = layout.height();

    let text_bytes = full_text.as_bytes();
    let mut runs = Vec::new();
    let mut line_infos = Vec::new();

    for (line_idx, line) in layout.lines().enumerate() {
        let metrics = line.metrics();
        let line_range = line.text_range();

        let start_byte = line_range.start;
        // `end_byte` is the caret position past the last visible char: strip
        // any trailing `\n` that parley may have included in the line range.
        let mut end_byte = line_range.end.min(full_text.len());
        while end_byte > start_byte && text_bytes.get(end_byte - 1) == Some(&b'\n') {
            end_byte -= 1;
        }
        // parley sometimes leaves the cursor stop on the empty trailing line
        // of a text ending in `\n`; keep start within bounds.
        let start_byte = start_byte.min(full_text.len());

        let mut stops: Vec<LineGlyphStop> = Vec::new();
        let mut right_x = 0.0f32;

        // The glyph→source-byte mapping is tracked with a single per-LINE
        // char cursor, advanced once per glyph across all glyph runs. In
        // parley 0.9 a `Run` exposes the *line's* `text_range`/`clusters` (via
        // `line_data`), so every glyph run in the line reports the same
        // whole-line range; the previous per-run `char_indices()` restarted at
        // byte 0 for each run and mis-tagged every glyph past the first style
        // run. That made clicks on later spans (e.g. a highlighted string in
        // the code editor) land on the wrong byte, so Backspace deleted the
        // wrong character. `line.items()` yields glyph runs in visual order
        // and tiles the line's glyphs without gaps, so walking one char cursor
        // in lockstep yields the correct byte for each glyph (for the
        // monospace, 1-char-per-glyph editor this is exact).
        let mut line_chars = full_text[start_byte..end_byte].char_indices();

        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let run = glyph_run.run();
            let font = run.font().clone();
            let font_size = run.font_size();
            let normalized_coords = run.normalized_coords().to_vec();
            let style = glyph_run.style();

            let run_range = run.text_range();
            let run_underline = underline_ranges
                .iter()
                .any(|&(start, end)| run_range.start < end && run_range.end > start);

            let mut glyphs = Vec::new();
            let mut x = glyph_run.offset();
            let y = glyph_run.baseline();

            for glyph in glyph_run.glyphs() {
                let gx = x + glyph.x;
                let gy = y - glyph.y;
                x += glyph.advance;
                let byte = line_chars
                    .next()
                    .map(|(off, _)| start_byte + off)
                    .unwrap_or(end_byte);
                right_x = right_x.max(gx + glyph.advance);
                stops.push(LineGlyphStop {
                    byte,
                    x: gx,
                    y: gy,
                    advance: glyph.advance,
                });
                glyphs.push(TextGlyph {
                    id: glyph.id,
                    x: gx,
                    y: gy,
                    advance: glyph.advance,
                });
            }

            runs.push(TextRunData {
                font,
                font_size,
                normalized_coords,
                glyphs,
                brush: style.brush,
                underline: run_underline,
                line_index: line_idx,
            });
        }

        line_infos.push(LineInfo {
            top: metrics.baseline - metrics.ascent,
            height: metrics.size(),
            baseline: metrics.baseline,
            start_byte,
            end_byte,
            right_x,
            stops,
        });
    }

    (
        TextLayoutData {
            runs,
            line_infos,
            _width: width,
            _height: height,
        },
        width,
        height,
    )
}
