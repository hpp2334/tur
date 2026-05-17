use std::fmt;

use tur_shared::{Color, Geometry, Offset};
use vello::kurbo::{Affine, Stroke};
use vello::peniko::{Brush, Fill, Image};
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

            if run.underline {
                if let Some(first) = run.glyphs.first() {
                    let last_x = run
                        .glyphs
                        .last()
                        .map(|g| g.x + g.advance)
                        .unwrap_or(first.x + first.advance);
                    let underline_y = first.y + run.font_size * 0.15;
                    let underline_brush = Brush::Solid(brush_color);
                    self.scene.stroke(
                        &Stroke::new(1.0),
                        transform,
                        &underline_brush,
                        None,
                        &vello::kurbo::Line::new(
                            (first.x as f64, underline_y as f64),
                            (last_x as f64, underline_y as f64),
                        ),
                    );
                }
            }
        }
    }

    fn draw_image(&mut self, image: &Image, transform: Affine) {
        self.scene.draw_image(image, transform);
    }

    fn stroke_geometry(
        &mut self,
        offset: Offset,
        geometry: &Geometry,
        color: &Color,
        stroke_width: f64,
    ) {
        let peniko_color = to_peniko_color(color);
        let transform = Affine::translate((offset.x, offset.y));
        let brush = Brush::Solid(peniko_color);
        let stroke = Stroke::new(stroke_width);
        match geometry {
            Geometry::Rect(size) => {
                self.scene.stroke(
                    &stroke,
                    transform,
                    &brush,
                    None,
                    &vello::kurbo::Rect::new(0.0, 0.0, size.width, size.height),
                );
            }
            Geometry::RoundedRect { size, radius } => {
                self.scene.stroke(
                    &stroke,
                    transform,
                    &brush,
                    None,
                    &vello::kurbo::RoundedRect::new(0.0, 0.0, size.width, size.height, *radius),
                );
            }
            Geometry::Circle { radius } => {
                self.scene.stroke(
                    &stroke,
                    transform,
                    &brush,
                    None,
                    &vello::kurbo::Circle::new((0.0, 0.0), *radius),
                );
            }
        }
    }
}

fn to_peniko_color(color: &Color) -> vello::peniko::Color {
    vello::peniko::Color::from_rgba8(color.r(), color.g(), color.b(), color.a())
}
