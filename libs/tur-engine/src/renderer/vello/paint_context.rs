use std::fmt;

use tur_shared::{Brush, Color, Geometry, Offset, Size};
use vello::kurbo::{Affine, Rect, Stroke};
use vello::peniko::{BlendMode, Brush as PenikoBrush, Fill, ImageData};
use vello::Scene;

use crate::core::render::Canvas;
use crate::core::text::text_layout::TextLayoutData;

pub struct VelloPaintContext<'a> {
    scene: &'a mut Scene,
    /// Accumulated affine transform applied to every draw call.
    ///
    /// Vello's `push_layer` transform only applies to the clip shape, not to
    /// the content drawn within the layer (see vello docs: "the transforms
    /// are _not_ saved or modified by the layer stack"). So to actually
    /// transform a subtree, we must bake the affine into each draw call's own
    /// transform. This stack holds the running product; `push_transform`
    /// composes onto it, and every draw/clip/opacity op premultiplies by it.
    transform_stack: Vec<Affine>,
}

impl<'a> VelloPaintContext<'a> {
    pub fn new(scene: &'a mut Scene) -> Self {
        VelloPaintContext {
            scene,
            transform_stack: Vec::new(),
        }
    }

    /// The current accumulated transform (IDENTITY when no transform layer is
    /// active). Draw calls premultiply their own translate/transform by this.
    fn current_transform(&self) -> Affine {
        self.transform_stack
            .last()
            .copied()
            .unwrap_or(Affine::IDENTITY)
    }
}

impl fmt::Debug for VelloPaintContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VelloPaintContext").finish()
    }
}

fn fill_shape(scene: &mut Scene, transform: Affine, geometry: &Geometry, brush: &PenikoBrush) {
    match geometry {
        Geometry::Rect(size) => {
            scene.fill(
                Fill::NonZero,
                transform,
                brush,
                None,
                &vello::kurbo::Rect::new(0.0, 0.0, size.width, size.height),
            );
        }
        Geometry::RoundedRect { size, radius } => {
            scene.fill(
                Fill::NonZero,
                transform,
                brush,
                None,
                &vello::kurbo::RoundedRect::new(0.0, 0.0, size.width, size.height, *radius),
            );
        }
        Geometry::Circle { radius } => {
            scene.fill(
                Fill::NonZero,
                transform,
                brush,
                None,
                &vello::kurbo::Circle::new((0.0, 0.0), *radius),
            );
        }
    }
}

fn stroke_shape(
    scene: &mut Scene,
    transform: Affine,
    geometry: &Geometry,
    brush: &PenikoBrush,
    stroke_width: f64,
) {
    let stroke = Stroke::new(stroke_width);
    match geometry {
        Geometry::Rect(size) => {
            scene.stroke(
                &stroke,
                transform,
                brush,
                None,
                &vello::kurbo::Rect::new(0.0, 0.0, size.width, size.height),
            );
        }
        Geometry::RoundedRect { size, radius } => {
            scene.stroke(
                &stroke,
                transform,
                brush,
                None,
                &vello::kurbo::RoundedRect::new(0.0, 0.0, size.width, size.height, *radius),
            );
        }
        Geometry::Circle { radius } => {
            scene.stroke(
                &stroke,
                transform,
                brush,
                None,
                &vello::kurbo::Circle::new((0.0, 0.0), *radius),
            );
        }
    }
}

fn geometry_size(geometry: &Geometry) -> Size {
    match geometry {
        Geometry::Rect(size) => *size,
        Geometry::RoundedRect { size, .. } => *size,
        Geometry::Circle { radius } => Size::new(radius * 2.0, radius * 2.0),
    }
}

impl Canvas for VelloPaintContext<'_> {
    fn fill_geometry(&mut self, offset: Offset, geometry: &Geometry, brush: &Brush) {
        let transform = self.current_transform() * Affine::translate((offset.x, offset.y));
        match brush {
            Brush::SolidColor(color) => {
                let peniko_color = to_peniko_color(color);
                let peniko_brush = PenikoBrush::Solid(peniko_color);
                fill_shape(self.scene, transform, geometry, &peniko_brush);
            }
            Brush::LinearGradient {
                start,
                end,
                stops,
            } => {
                let size = geometry_size(geometry);
                let x0 = start.0 * size.width;
                let y0 = start.1 * size.height;
                let x1 = end.0 * size.width;
                let y1 = end.1 * size.height;
                let peniko_stops: Vec<(f32, vello::peniko::Color)> = stops
                    .iter()
                    .map(|s| (s.offset, to_peniko_color(&s.color)))
                    .collect();
                let gradient =
                    vello::peniko::Gradient::new_linear((x0, y0), (x1, y1))
                        .with_stops(peniko_stops.as_slice());
                fill_shape(self.scene, transform, geometry, &PenikoBrush::Gradient(gradient));
            }
        }
    }

    #[allow(private_interfaces)]
    fn fill_text_layout(&mut self, offset: Offset, layout: &TextLayoutData) {
        let transform = self.current_transform() * Affine::translate((offset.x, offset.y));
        for run in &layout.runs {
            let brush_color = vello::peniko::Color::from_rgba8(
                run.brush[0],
                run.brush[1],
                run.brush[2],
                run.brush[3],
            );
            self.scene
                .draw_glyphs(&run.font)
                .brush(&PenikoBrush::Solid(brush_color))
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
                    let underline_brush = PenikoBrush::Solid(brush_color);
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

    fn draw_image(&mut self, image: &ImageData, transform: Affine) {
        let transform = self.current_transform() * transform;
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
        let transform = self.current_transform() * Affine::translate((offset.x, offset.y));
        let brush = PenikoBrush::Solid(peniko_color);
        stroke_shape(self.scene, transform, geometry, &brush, stroke_width);
    }

    fn draw_shadow(
        &mut self,
        offset: Offset,
        size: Size,
        color: &Color,
        border_radius: f64,
        blur: f64,
        shadow_offset: (f64, f64),
    ) {
        let peniko_color = to_peniko_color(color);
        let transform = self.current_transform()
            * Affine::translate((
                offset.x + shadow_offset.0,
                offset.y + shadow_offset.1,
            ));
        let rect = vello::kurbo::Rect::new(0.0, 0.0, size.width, size.height);
        self.scene.draw_blurred_rounded_rect(
            transform,
            rect,
            peniko_color,
            border_radius,
            blur,
        );
    }

    fn push_clip(&mut self, offset: Offset, size: Size) {
        let transform = self.current_transform() * Affine::translate((offset.x, offset.y));
        let clip = Rect::new(0.0, 0.0, size.width, size.height);
        self.scene.push_layer(Fill::NonZero, BlendMode::default(), 1.0, transform, &clip);
    }

    fn pop_clip(&mut self) {
        self.scene.pop_layer();
    }

    fn push_opacity(&mut self, opacity: f32) {
        // Push a layer with a near-infinite clip and reduced alpha. Vello
        // composites the layer contents with the given opacity when
        // pop_layer is called. The clip transform carries the current
        // transform so the (near-infinite) clip stays aligned with content
        // drawn inside a transform subtree.
        let opacity = opacity.clamp(0.0, 1.0);
        let transform = self.current_transform();
        let huge_clip = Rect::new(-1e6, -1e6, 1e6, 1e6);
        self.scene.push_layer(
            Fill::NonZero,
            BlendMode::default(),
            opacity,
            transform,
            &huge_clip,
        );
    }

    fn pop_opacity(&mut self) {
        self.scene.pop_layer();
    }

    fn push_transform(&mut self, transform: Affine) {
        // Vello layers do not transform their content (only their clip), so
        // we compose the affine onto an internal stack and bake it into each
        // subsequent draw call's own transform. No push_layer is needed for a
        // pure affine — vector content renders correctly per-draw.
        let next = self.current_transform() * transform;
        self.transform_stack.push(next);
    }

    fn pop_transform(&mut self) {
        self.transform_stack.pop();
    }
}

fn to_peniko_color(color: &Color) -> vello::peniko::Color {
    vello::peniko::Color::from_rgba8(color.r(), color.g(), color.b(), color.a())
}
