use parley::layout::PositionedLayoutItem;

pub(crate) struct LineInfo {
    pub top: f32,
    pub height: f32,
    pub baseline: f32,
    #[allow(dead_code)]
    pub start_char: usize,
    pub glyph_count: usize,
}

pub(crate) struct TextLayoutData {
    pub runs: Vec<TextRunData>,
    pub line_infos: Vec<LineInfo>,
    pub _width: f32,
    pub _height: f32,
}

pub(crate) struct TextRunData {
    pub font: vello::peniko::Font,
    pub font_size: f32,
    pub normalized_coords: Vec<parley::swash::NormalizedCoord>,
    pub glyphs: Vec<TextGlyph>,
    pub brush: [u8; 4],
    pub underline: bool,
    #[allow(dead_code)]
    pub line_index: usize,
}

pub(crate) struct TextGlyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

impl TextLayoutData {
    pub fn cursor_x_at(&self, char_index: usize) -> f32 {
        let mut chars_seen = 0;
        for run in &self.runs {
            for glyph in &run.glyphs {
                if chars_seen == char_index {
                    return glyph.x;
                }
                chars_seen += 1;
            }
        }
        self.runs
            .last()
            .and_then(|r| r.glyphs.last())
            .map(|g| g.x + g.advance)
            .unwrap_or(0.0)
    }

    pub fn cursor_xy_at(&self, char_index: usize) -> (f32, f32) {
        let mut chars_seen = 0;
        for run in &self.runs {
            for glyph in &run.glyphs {
                if chars_seen == char_index {
                    return (glyph.x, glyph.y);
                }
                chars_seen += 1;
            }
        }
        self.runs
            .last()
            .and_then(|r| r.glyphs.last())
            .map(|g| (g.x + g.advance, g.y))
            .unwrap_or((0.0, 0.0))
    }

    pub fn line_index_at_y(&self, y: f32) -> usize {
        for (i, line) in self.line_infos.iter().enumerate() {
            if y < line.top + line.height {
                return i;
            }
        }
        self.line_infos.len().saturating_sub(1)
    }

    pub fn line_index_for_char(&self, char_index: usize) -> usize {
        let mut chars_seen = 0;
        for (i, line) in self.line_infos.iter().enumerate() {
            if char_index < chars_seen + line.glyph_count {
                return i;
            }
            chars_seen += line.glyph_count;
        }
        self.line_infos.len().saturating_sub(1)
    }

    pub fn line_start_char(&self, line_index: usize) -> usize {
        if line_index == 0 {
            return 0;
        }
        self.line_infos
            .iter()
            .take(line_index)
            .map(|l| l.glyph_count)
            .sum()
    }

    pub fn line_end_char(&self, line_index: usize) -> usize {
        self.line_infos
            .iter()
            .take(line_index + 1)
            .map(|l| l.glyph_count)
            .sum()
    }

    #[allow(dead_code)]
    pub fn line_count(&self) -> usize {
        self.line_infos.len()
    }

    pub fn char_index_at_x(&self, x: f32) -> usize {
        let mut chars_seen = 0;
        let mut total_chars = 0;
        for run in &self.runs {
            total_chars += run.glyphs.len();
            for glyph in &run.glyphs {
                let glyph_center = glyph.x + glyph.advance / 2.0;
                if x <= glyph_center {
                    return chars_seen;
                }
                chars_seen += 1;
            }
        }
        total_chars
    }

    pub fn char_index_at_xy(&self, x: f32, y: f32) -> usize {
        let line_idx = self.line_index_at_y(y);
        let start = self.line_start_char(line_idx);
        let end = self.line_end_char(line_idx);

        let mut chars_seen = 0;
        for run in &self.runs {
            for glyph in &run.glyphs {
                if chars_seen >= end {
                    return end;
                }
                if chars_seen >= start {
                    let glyph_center = glyph.x + glyph.advance / 2.0;
                    if x <= glyph_center {
                        return chars_seen;
                    }
                }
                chars_seen += 1;
            }
        }
        end
    }

    pub fn line_height_at(&self, line_index: usize) -> f32 {
        self.line_infos
            .get(line_index)
            .map(|l| l.height)
            .unwrap_or(0.0)
    }
}

pub(crate) fn extract_layout_data(
    layout: &mut parley::Layout<[u8; 4]>,
    underline_ranges: &[(usize, usize)],
) -> (TextLayoutData, f32, f32) {
    let width = layout.width();
    let height = layout.height();

    let mut char_offset = 0usize;
    let mut runs = Vec::new();
    let mut line_infos = Vec::new();

    for (line_idx, line) in layout.lines().enumerate() {
        let metrics = line.metrics();
        let line_start_char = char_offset;
        let mut line_glyph_count = 0usize;

        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let run = glyph_run.run();
            let font = run.font().clone();
            let font_size = run.font_size();
            let normalized_coords = run.normalized_coords().to_vec();
            let style = glyph_run.style();

            let glyph_count = glyph_run.glyphs().count();

            let run_underline = underline_ranges
                .iter()
                .any(|&(start, end)| char_offset < end && char_offset + glyph_count > start);

            let mut glyphs = Vec::new();
            let mut x = glyph_run.offset();
            let y = glyph_run.baseline();
            for glyph in glyph_run.glyphs() {
                let gx = x + glyph.x;
                let gy = y - glyph.y;
                x += glyph.advance;
                glyphs.push(TextGlyph {
                    id: glyph.id as u32,
                    x: gx,
                    y: gy,
                    advance: glyph.advance,
                });
            }

            char_offset += glyph_count;
            line_glyph_count += glyph_count;

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
            start_char: line_start_char,
            glyph_count: line_glyph_count,
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
