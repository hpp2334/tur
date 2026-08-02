/// One user-positionable glyph stop on a line, in source-string BYTE space.
///
/// `byte` is the byte offset in the source string where this glyph starts.
/// This is the single source of truth that keeps the layout's geometry (x/y)
/// aligned with the `TextEditingController`'s byte-based cursor/selection
/// offsets — newlines (`\n`) consume a byte in the string but produce no
/// glyph, so we must never count glyphs as if they were string indices.
#[derive(Copy, Clone, Debug)]
pub struct LineGlyphStop {
    pub byte: usize,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

#[derive(Debug)]
pub struct LineInfo {
    pub top: f32,
    pub height: f32,
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

#[derive(Debug)]
pub struct TextLayoutData {
    pub runs: Vec<TextRunData>,
    pub line_infos: Vec<LineInfo>,
    pub _width: f32,
    pub _height: f32,
}

#[derive(Debug)]
pub struct TextRunData {
    pub font: parley::FontData,
    pub font_size: f32,
    pub normalized_coords: Vec<i16>,
    pub glyphs: Vec<TextGlyph>,
    pub brush: [u8; 4],
    pub underline: bool,
    pub line_index: usize,
}

#[derive(Debug)]
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
