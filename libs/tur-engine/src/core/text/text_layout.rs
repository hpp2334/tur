use parley::layout::PositionedLayoutItem;

/// One user-positionable glyph stop on a line, in source-string BYTE space.
///
/// `byte` is the byte offset in the source string where this glyph starts.
/// This is the single source of truth that keeps the layout's geometry (x/y)
/// aligned with the `TextEditingController`'s byte-based cursor/selection
/// offsets — newlines (`\n`) consume a byte in the string but produce no
/// glyph, so we must never count glyphs as if they were string indices.
#[derive(Copy, Clone)]
pub struct LineGlyphStop {
    pub byte: usize,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

pub struct LineInfo {
    pub top: f32,
    pub height: f32,
    #[allow(dead_code)]
    pub baseline: f32,
    /// Byte offset of the first character of this line (after the previous
    /// line's terminating `\n`, or 0 for the first line).
    pub start_byte: usize,
    /// Byte offset of the cursor sitting at the END of this line's visible
    /// text — i.e. just past the last non-newline character. For a line ended
    /// by `\n`, this points AT the `\n` (cursor before it). For the final
    /// line, this is the full text length.
    pub end_byte: usize,
    /// x of the right edge of the last glyph on this line (where the cursor
    /// sits at `end_byte`).
    pub right_x: f32,
    /// Glyphs on this line, in visual order, each tagged with its source byte.
    pub stops: Vec<LineGlyphStop>,
}

pub struct TextLayoutData {
    pub runs: Vec<TextRunData>,
    pub line_infos: Vec<LineInfo>,
    pub _width: f32,
    pub _height: f32,
}

pub struct TextRunData {
    pub font: parley::FontData,
    pub font_size: f32,
    pub normalized_coords: Vec<i16>,
    pub glyphs: Vec<TextGlyph>,
    pub brush: [u8; 4],
    pub underline: bool,
    #[allow(dead_code)]
    pub line_index: usize,
}

pub struct TextGlyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

impl TextLayoutData {
    /// x of the cursor at `byte` (single-line convenience).
    pub fn cursor_x_at(&self, byte: usize) -> f32 {
        self.cursor_xy_at(byte).0
    }

    /// (x, y) of the cursor caret at the given source-string byte offset.
    pub fn cursor_xy_at(&self, byte: usize) -> (f32, f32) {
        let line = self.line_index_for_byte(byte);
        let info = &self.line_infos[line];
        let b = byte.clamp(info.start_byte, info.end_byte);

        // Exact glyph at this byte?
        for s in &info.stops {
            if s.byte == b {
                return (s.x, s.y);
            }
        }
        // b == start_byte with no matching glyph (empty line / before first
        // glyph): caret at the line's left edge.
        if b <= info.start_byte {
            if let Some(first) = info.stops.first() {
                return (first.x, first.y);
            }
            return (0.0, info.baseline);
        }
        // b == end_byte (after the last glyph): caret at the line's right edge.
        if let Some(last) = info.stops.last() {
            return (last.x + last.advance, last.y);
        }
        (0.0, info.baseline)
    }

    pub fn line_index_at_y(&self, y: f32) -> usize {
        for (i, line) in self.line_infos.iter().enumerate() {
            if y < line.top + line.height {
                return i;
            }
        }
        self.line_infos.len().saturating_sub(1)
    }

    /// Index of the layout line containing the given source-string byte.
    /// A `\n` byte belongs to the line it terminates.
    pub fn line_index_for_byte(&self, byte: usize) -> usize {
        let mut idx = 0;
        for (i, info) in self.line_infos.iter().enumerate() {
            if byte >= info.start_byte {
                idx = i;
            } else {
                break;
            }
        }
        idx
    }

    pub fn line_start_byte(&self, line_index: usize) -> usize {
        self.line_infos
            .get(line_index)
            .map(|l| l.start_byte)
            .unwrap_or(0)
    }

    pub fn line_end_byte(&self, line_index: usize) -> usize {
        self.line_infos
            .get(line_index)
            .map(|l| l.end_byte)
            .unwrap_or(0)
    }

    /// x of the right edge of the last glyph on the given line.
    pub fn line_right_x(&self, line_index: usize) -> f32 {
        self.line_infos
            .get(line_index)
            .map(|l| l.right_x)
            .unwrap_or(0.0)
    }

    #[allow(dead_code)]
    pub fn line_count(&self) -> usize {
        self.line_infos.len()
    }

    /// Single-line hit test: byte offset at the given x on line 0.
    pub fn byte_index_at_x(&self, x: f32) -> usize {
        let Some(info) = self.line_infos.first() else {
            return 0;
        };
        byte_at_x(info, x)
    }

    /// Multi-line hit test: byte offset at the given (x, y).
    pub fn byte_index_at_xy(&self, x: f32, y: f32) -> usize {
        let line = self.line_index_at_y(y);
        let Some(info) = self.line_infos.get(line) else {
            return 0;
        };
        byte_at_x(info, x)
    }
}

fn byte_at_x(info: &LineInfo, x: f32) -> usize {
    for s in &info.stops {
        let center = s.x + s.advance / 2.0;
        if x <= center {
            return s.byte;
        }
    }
    info.end_byte
}

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
