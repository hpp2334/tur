use std::fmt;

use tur_shared::{Color, Geometry, Offset};
use vello::kurbo::Affine;
use vello::peniko::{Brush, Fill};
use vello::Scene;

use crate::core::render::Canvas;

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

impl Canvas for VelloPaintContext<'_> {
    fn fill_geometry(&mut self, offset: Offset, geometry: &Geometry, color: &Color) {
        let peniko_color = to_peniko_color(color);
        let transform = Affine::translate((offset.x, offset.y));
        let brush = Brush::Solid(peniko_color);
        match geometry {
            Geometry::Rect(size) => {
                self.scene.fill(
                    Fill::NonZero,
                    transform,
                    &brush,
                    None,
                    &vello::kurbo::Rect::new(0.0, 0.0, size.width, size.height),
                );
            }
            Geometry::RoundedRect { size, radius } => {
                self.scene.fill(
                    Fill::NonZero,
                    transform,
                    &brush,
                    None,
                    &vello::kurbo::RoundedRect::new(0.0, 0.0, size.width, size.height, *radius),
                );
            }
            Geometry::Circle { radius } => {
                self.scene.fill(
                    Fill::NonZero,
                    transform,
                    &brush,
                    None,
                    &vello::kurbo::Circle::new((0.0, 0.0), *radius),
                );
            }
        }
    }

    fn fill_text(&mut self, offset: Offset, _text: &str, _font_size: f64, color: &Color) {
        let peniko_color = to_peniko_color(color);
        let transform = Affine::translate((offset.x, offset.y));
        self.scene.fill(
            Fill::NonZero,
            transform,
            &Brush::Solid(peniko_color),
            None,
            &vello::kurbo::Rect::new(0.0, 0.0, 0.0, 0.0),
        );
    }
}

fn to_peniko_color(color: &Color) -> vello::peniko::Color {
    vello::peniko::Color::from_rgba8(color.r(), color.g(), color.b(), color.a())
}
