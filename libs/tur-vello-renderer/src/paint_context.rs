use std::fmt;

use tur_render_tree::PaintContext as TurPaintContext;
use tur_render_tree::{Offset, Size};
use vello::kurbo::Affine;
use vello::peniko::{Brush, Color, Fill};
use vello::Scene;

pub struct VelloPaintContext<'a> {
    scene: &'a mut Scene,
}

impl<'a> VelloPaintContext<'a> {
    pub fn new(scene: &'a mut Scene) -> Self {
        VelloPaintContext { scene }
    }
}

impl fmt::Debug for VelloPaintContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VelloPaintContext").finish()
    }
}

impl TurPaintContext for VelloPaintContext<'_> {
    fn fill_rect(&mut self, offset: Offset, size: Size, color: &str) {
        let color = parse_color(color);
        let transform = Affine::translate((offset.x, offset.y));
        self.scene.fill(
            Fill::NonZero,
            transform,
            &Brush::Solid(color),
            None,
            &vello::kurbo::Rect::new(0.0, 0.0, size.width, size.height),
        );
    }

    fn fill_text(&mut self, offset: Offset, _text: &str, _font_size: f64, color: &str) {
        let color = parse_color(color);
        let transform = Affine::translate((offset.x, offset.y));
        self.scene.fill(
            Fill::NonZero,
            transform,
            &Brush::Solid(color),
            None,
            &vello::kurbo::Rect::new(0.0, 0.0, 0.0, 0.0),
        );
    }
}

fn parse_color(s: &str) -> Color {
    let hex = s.trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            Color::from_rgba8(r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            Color::from_rgba8(r, g, b, a)
        }
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(0);
            Color::from_rgba8(r, g, b, 255)
        }
        _ => Color::from_rgba8(255, 255, 255, 255),
    }
}
