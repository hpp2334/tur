use std::fmt;

use tur_shared::{Color, Geometry, Offset};
use vello::kurbo::Affine;
use vello::peniko::{Brush, Fill};
use vello::Scene;

use crate::core::render::Canvas;
use crate::elements::text::text_layout::TextLayoutData;

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
        tracing::info!(
            "fill_geometry: offset={:?} geometry={:?} color={:?}",
            offset,
            geometry,
            peniko_color
        );
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

    #[allow(private_interfaces)]
    fn fill_text_layout(&mut self, offset: Offset, layout: &TextLayoutData) {
        let transform = Affine::translate((offset.x, offset.y));
        for run in &layout.runs {
            let brush_color = vello::peniko::Color::from_rgba8(
                run.brush[0],
                run.brush[1],
                run.brush[2],
                run.brush[3],
            );
            self.scene
                .draw_glyphs(&run.font)
                .brush(&Brush::Solid(brush_color))
                .font_size(run.font_size)
                .normalized_coords(&run.normalized_coords)
                .transform(transform)
                .draw(
                    Fill::NonZero,
                    run.glyphs.iter().map(|g| vello::Glyph {
                        id: g.id,
                        x: g.x,
                        y: g.y,
                    }),
                );
        }
    }
}

fn to_peniko_color(color: &Color) -> vello::peniko::Color {
    vello::peniko::Color::from_rgba8(color.r(), color.g(), color.b(), color.a())
}
