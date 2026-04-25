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
}
