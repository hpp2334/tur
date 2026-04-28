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
