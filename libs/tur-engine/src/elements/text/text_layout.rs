use parley::layout::PositionedLayoutItem;

pub(crate) struct TextLayoutData {
    pub runs: Vec<TextRunData>,
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
}

pub(crate) fn extract_layout_data(
    layout: &mut parley::Layout<[u8; 4]>,
    underline_ranges: &[(usize, usize)],
) -> (TextLayoutData, f32, f32) {
    let width = layout.width();
    let height = layout.height();

    let mut char_offset = 0usize;
    let mut runs = Vec::new();
    for line in layout.lines() {
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

            runs.push(TextRunData {
                font,
                font_size,
                normalized_coords,
                glyphs,
                brush: style.brush,
                underline: run_underline,
            });
        }
    }

    (
        TextLayoutData {
            runs,
            _width: width,
            _height: height,
        },
        width,
        height,
    )
}
