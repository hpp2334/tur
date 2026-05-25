use tur_shared::Size;
use vello::kurbo::{Affine, BezPath, Point, Stroke};
use vello::peniko::{Brush, Color, ColorStop, Fill};
use vello::Scene;

pub struct SvgResource {
    pub scene: Scene,
    pub natural_size: Size,
}

impl SvgResource {
    pub fn parse(svg_str: &str) -> Option<Self> {
        let tree = usvg::Tree::from_str(svg_str, &usvg::Options::default()).ok()?;
        let size = tree.size();
        let natural_size = Size::new(size.width() as f64, size.height() as f64);
        let mut scene = Scene::new();
        render_group(&mut scene, tree.root(), Affine::IDENTITY);
        Some(SvgResource {
            scene,
            natural_size,
        })
    }
}

fn render_group(scene: &mut Scene, group: &usvg::Group, transform: Affine) {
    for node in group.children() {
        let transform = transform * to_affine(&node.abs_transform());
        match node {
            usvg::Node::Group(g) => {
                let alpha = g.opacity().get();
                let blend_mode: vello::peniko::BlendMode = match g.blend_mode() {
                    usvg::BlendMode::Normal => vello::peniko::Mix::Normal.into(),
                    usvg::BlendMode::Multiply => vello::peniko::Mix::Multiply.into(),
                    usvg::BlendMode::Screen => vello::peniko::Mix::Screen.into(),
                    usvg::BlendMode::Overlay => vello::peniko::Mix::Overlay.into(),
                    usvg::BlendMode::Darken => vello::peniko::Mix::Darken.into(),
                    usvg::BlendMode::Lighten => vello::peniko::Mix::Lighten.into(),
                    usvg::BlendMode::ColorDodge => vello::peniko::Mix::ColorDodge.into(),
                    usvg::BlendMode::ColorBurn => vello::peniko::Mix::ColorBurn.into(),
                    usvg::BlendMode::HardLight => vello::peniko::Mix::HardLight.into(),
                    usvg::BlendMode::SoftLight => vello::peniko::Mix::SoftLight.into(),
                    usvg::BlendMode::Difference => vello::peniko::Mix::Difference.into(),
                    usvg::BlendMode::Exclusion => vello::peniko::Mix::Exclusion.into(),
                    usvg::BlendMode::Hue => vello::peniko::Mix::Hue.into(),
                    usvg::BlendMode::Saturation => vello::peniko::Mix::Saturation.into(),
                    usvg::BlendMode::Color => vello::peniko::Mix::Color.into(),
                    usvg::BlendMode::Luminosity => vello::peniko::Mix::Luminosity.into(),
                };

                let clipped = match g
                    .clip_path()
                    .and_then(|path| path.root().children().first())
                {
                    Some(usvg::Node::Path(clip_path)) => {
                        let local_path = to_bez_path(clip_path);
                        scene.push_layer(Fill::NonZero, blend_mode, alpha, transform, &local_path);
                        true
                    }
                    _ => {
                        let bb = g.layer_bounding_box();
                        let rect = vello::kurbo::Rect::from_origin_size(
                            (bb.x(), bb.y()),
                            (bb.width() as f64, bb.height() as f64),
                        );
                        scene.push_layer(Fill::NonZero, blend_mode, alpha, transform, &rect);
                        true
                    }
                };

                render_group(scene, g, Affine::IDENTITY);

                if clipped {
                    scene.pop_layer();
                }
            }
            usvg::Node::Path(path) => {
                if !path.is_visible() {
                    continue;
                }
                let local_path = to_bez_path(path);

                if let Some(fill) = &path.fill() {
                    if let Some((brush, brush_transform)) = to_brush(fill.paint(), fill.opacity()) {
                        scene.fill(
                            match fill.rule() {
                                usvg::FillRule::NonZero => Fill::NonZero,
                                usvg::FillRule::EvenOdd => Fill::EvenOdd,
                            },
                            transform,
                            &brush,
                            Some(brush_transform),
                            &local_path,
                        );
                    }
                }
                if let Some(stroke) = &path.stroke() {
                    if let Some((brush, brush_transform)) = to_brush(stroke.paint(), stroke.opacity()) {
                        let conv_stroke = to_stroke(stroke);
                        scene.stroke(
                            &conv_stroke,
                            transform,
                            &brush,
                            Some(brush_transform),
                            &local_path,
                        );
                    }
                }
            }
            usvg::Node::Text(text) => {
                render_group(scene, text.flattened(), transform);
            }
            usvg::Node::Image(_) => {}
        }
    }
}

fn to_affine(ts: &usvg::Transform) -> Affine {
    let usvg::Transform { sx, kx, ky, sy, tx, ty } = ts;
    Affine::new([sx, ky, kx, sy, tx, ty].map(|&x| f64::from(x)))
}

fn to_stroke(stroke: &usvg::Stroke) -> Stroke {
    let mut conv = Stroke::new(stroke.width().get() as f64)
        .with_caps(match stroke.linecap() {
            usvg::LineCap::Butt => vello::kurbo::Cap::Butt,
            usvg::LineCap::Round => vello::kurbo::Cap::Round,
            usvg::LineCap::Square => vello::kurbo::Cap::Square,
        })
        .with_join(match stroke.linejoin() {
            usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => vello::kurbo::Join::Miter,
            usvg::LineJoin::Round => vello::kurbo::Join::Round,
            usvg::LineJoin::Bevel => vello::kurbo::Join::Bevel,
        })
        .with_miter_limit(stroke.miterlimit().get() as f64);
    if let Some(dash_array) = stroke.dasharray().as_ref() {
        conv = conv.with_dashes(
            stroke.dashoffset() as f64,
            dash_array.iter().map(|x| *x as f64),
        );
    }
    conv
}

fn to_bez_path(path: &usvg::Path) -> BezPath {
    let mut local_path = BezPath::new();
    let mut just_closed = false;
    let mut most_recent_initial = (0.0, 0.0);
    for elt in path.data().segments() {
        match elt {
            usvg::tiny_skia_path::PathSegment::MoveTo(p) => {
                if std::mem::take(&mut just_closed) {
                    local_path.move_to(most_recent_initial);
                }
                most_recent_initial = (p.x.into(), p.y.into());
                local_path.move_to(most_recent_initial);
            }
            usvg::tiny_skia_path::PathSegment::LineTo(p) => {
                if std::mem::take(&mut just_closed) {
                    local_path.move_to(most_recent_initial);
                }
                local_path.line_to(Point::new(p.x as f64, p.y as f64));
            }
            usvg::tiny_skia_path::PathSegment::QuadTo(p1, p2) => {
                if std::mem::take(&mut just_closed) {
                    local_path.move_to(most_recent_initial);
                }
                local_path.quad_to(
                    Point::new(p1.x as f64, p1.y as f64),
                    Point::new(p2.x as f64, p2.y as f64),
                );
            }
            usvg::tiny_skia_path::PathSegment::CubicTo(p1, p2, p3) => {
                if std::mem::take(&mut just_closed) {
                    local_path.move_to(most_recent_initial);
                }
                local_path.curve_to(
                    Point::new(p1.x as f64, p1.y as f64),
                    Point::new(p2.x as f64, p2.y as f64),
                    Point::new(p3.x as f64, p3.y as f64),
                );
            }
            usvg::tiny_skia_path::PathSegment::Close => {
                just_closed = true;
                local_path.close_path();
            }
        }
    }
    local_path
}

fn to_brush(paint: &usvg::Paint, opacity: usvg::Opacity) -> Option<(Brush, Affine)> {
    match paint {
        usvg::Paint::Color(color) => Some((
            Brush::Solid(Color::from_rgba8(
                color.red,
                color.green,
                color.blue,
                opacity.to_u8(),
            )),
            Affine::IDENTITY,
        )),
        usvg::Paint::LinearGradient(gr) => {
            let stops: Vec<ColorStop> = gr
                .stops()
                .iter()
                .map(|stop| ColorStop {
                    offset: stop.offset().get(),
                    color: vello::peniko::color::DynamicColor::from_alpha_color(
                        Color::from_rgba8(
                            stop.color().red,
                            stop.color().green,
                            stop.color().blue,
                            (stop.opacity() * opacity).to_u8(),
                        ),
                    ),
                })
                .collect();
            let start = Point::new(gr.x1() as f64, gr.y1() as f64);
            let end = Point::new(gr.x2() as f64, gr.y2() as f64);
            let arr = [
                gr.transform().sx,
                gr.transform().ky,
                gr.transform().kx,
                gr.transform().sy,
                gr.transform().tx,
                gr.transform().ty,
            ]
            .map(f64::from);
            let transform = Affine::new(arr);
            let gradient = vello::peniko::Gradient::new_linear(start, end)
                .with_stops(stops.as_slice());
            Some((Brush::Gradient(gradient), transform))
        }
        usvg::Paint::RadialGradient(gr) => {
            let stops: Vec<ColorStop> = gr
                .stops()
                .iter()
                .map(|stop| ColorStop {
                    offset: stop.offset().get(),
                    color: vello::peniko::color::DynamicColor::from_alpha_color(
                        Color::from_rgba8(
                            stop.color().red,
                            stop.color().green,
                            stop.color().blue,
                            (stop.opacity() * opacity).to_u8(),
                        ),
                    ),
                })
                .collect();
            let start_center = Point::new(gr.cx() as f64, gr.cy() as f64);
            let end_center = Point::new(gr.fx() as f64, gr.fy() as f64);
            let start_radius = 0.0_f32;
            let end_radius = gr.r().get();
            let arr = [
                gr.transform().sx,
                gr.transform().ky,
                gr.transform().kx,
                gr.transform().sy,
                gr.transform().tx,
                gr.transform().ty,
            ]
            .map(f64::from);
            let transform = Affine::new(arr);
            let gradient = vello::peniko::Gradient::new_two_point_radial(
                start_center,
                start_radius,
                end_center,
                end_radius,
            )
            .with_stops(stops.as_slice());
            Some((Brush::Gradient(gradient), transform))
        }
        usvg::Paint::Pattern(_) => None,
    }
}
