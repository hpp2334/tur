use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::core::layout::{Geometry, Offset, Size};
use crate::core::render::brush::{Brush, Color};
use glifo::Glyph;
use vello_common::kurbo::{Affine, BezPath, Circle, Line, Rect, RoundedRect, Shape, Stroke};
use vello_common::paint::{Image, ImageId, ImageSource, PaintType};
use vello_common::peniko::{BlendMode, Color as PenikoColor, Fill, Gradient};
use vello_hybrid::{Resources, Scene};

use crate::core::element::ElementNodeId;
use crate::core::image_resource::ImageResourceId;
use crate::core::render::Canvas;
use crate::core::text::text_layout::TextLayoutData;
use crate::renderer::vello::text_culling::{ClipMirror, local_clip_y, visible_line_range};

/// Tolerance used when converting non-rectangular shapes (rounded rects,
/// circles) into Bézier paths for the hybrid renderer.
const TOLERANCE: f64 = 0.1;

/// Cap on cached flattened paths — a UI draws a bounded set of distinct
/// shape geometries; past this, clear (a pathological per-frame-unique-shape
/// workload would otherwise grow the maps unboundedly).
const PATH_CACHE_CAP: usize = 1024;

/// Scratch state shared between a renderer and every paint context it
/// creates — per-frame allocation reuse for the scene-rebuild pass.
///
/// Flattened Bézier paths for rounded rects / circles / plain-rect clips are
/// cached by exact-geometry (f64-bit-pattern keys): the same shape drawn
/// every frame flattens once instead of per fill. The gradient-stop Vec is
/// reused across fills (`Gradient::with_stops` copies the stops, so the
/// scratch is safe to recycle). Owned by the renderer (host-thread);
/// borrowed `&mut` by each frame's [`VelloPaintContext`].
#[derive(Default, Debug)]
pub struct PaintScratch {
    rounded_rects: HashMap<(u64, u64, u64), BezPath>,
    circles: HashMap<u64, BezPath>,
    rects: HashMap<(u64, u64), BezPath>,
    gradient_stops: Vec<(f32, PenikoColor)>,
}

impl PaintScratch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Flattened path for a rounded rect (cached by `(w, h, r)` bits).
    fn rounded_rect_path(&mut self, width: f64, height: f64, radius: f64) -> &BezPath {
        let key = (width.to_bits(), height.to_bits(), radius.to_bits());
        if self.rounded_rects.len() >= PATH_CACHE_CAP {
            self.rounded_rects.clear();
        }
        self.rounded_rects
            .entry(key)
            .or_insert_with(|| RoundedRect::new(0.0, 0.0, width, height, radius).to_path(TOLERANCE))
    }

    /// Flattened path for a circle (cached by radius bits).
    fn circle_path(&mut self, radius: f64) -> &BezPath {
        let key = radius.to_bits();
        if self.circles.len() >= PATH_CACHE_CAP {
            self.circles.clear();
        }
        self.circles
            .entry(key)
            .or_insert_with(|| Circle::new((0.0, 0.0), radius).to_path(TOLERANCE))
    }

    /// Flattened path for an origin-aligned rect (a rect's `to_path` still
    /// allocates a BezPath per call otherwise — one per clip push per frame).
    /// Only origin-aligned rects (clips) are cached: a translated rect's path
    /// is its (0,0)-corner path under a different transform.
    fn rect_path(&mut self, width: f64, height: f64) -> &BezPath {
        let key = (width.to_bits(), height.to_bits());
        if self.rects.len() >= PATH_CACHE_CAP {
            self.rects.clear();
        }
        self.rects
            .entry(key)
            .or_insert_with(|| Rect::new(0.0, 0.0, width, height).to_path(TOLERANCE))
    }
}

pub struct VelloPaintContext<'a> {
    scene: &'a mut Scene,
    resources: &'a mut Resources,
    /// Maps each image resource id to its uploaded hybrid `ImageId`. The WebGPU
    /// backend only supports `ImageSource::OpaqueId`.
    image_uploads: &'a HashMap<ImageResourceId, ImageId>,
    /// Accumulated affine transform applied to every draw call.
    ///
    /// Vello Hybrid's `push_layer` only applies a transform to the clip shape,
    /// not to the content drawn within the layer (the scene has a single global
    /// `set_transform` state that layers do not save/restore). So to actually
    /// transform a subtree, we bake the affine into each draw call via
    /// `set_transform`. This stack holds the running product; `push_transform`
    /// composes onto it, and every draw/clip/opacity op premultiplies by it.
    transform_stack: Vec<Affine>,
    /// Mirror of the clip-layer stack as conservative scene-space AABBs
    /// (seeded with the surface rect). Pushed/popped alongside the real GPU
    /// clip layers — the same ops drive both, so they cannot diverge. Used
    /// to cull text glyph runs down to the visible line range (a tall
    /// editor inside a `ScrollView` must not re-encode its whole document
    /// every frame). See `text_culling`.
    clip_mirror: ClipMirror,
    /// Per-frame allocation reuse (flattened-shape path cache + gradient
    /// stop scratch) — owned by the renderer, borrowed per frame. See
    /// [`PaintScratch`].
    scratch: &'a mut PaintScratch,
}

impl<'a> VelloPaintContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scene: &'a mut Scene,
        resources: &'a mut Resources,
        root_transform: Affine,
        image_uploads: &'a HashMap<ImageResourceId, ImageId>,
        surface: Rect,
        scratch: &'a mut PaintScratch,
    ) -> Self {
        // Seed the transform stack with the root transform (the dpr scale). The
        // hybrid scene has a single global transform state that layers do not
        // save/restore, so every draw call bakes `current_transform()` into its
        // own `set_transform`.
        VelloPaintContext {
            scene,
            resources,
            image_uploads,
            transform_stack: vec![root_transform],
            clip_mirror: ClipMirror::new(surface),
            scratch,
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

/// Build the hybrid `PaintType` for a tur `Brush`. Gradient stops flow
/// through the scratch Vec (refilled per fill; `Gradient::with_stops`
/// copies them, so the scratch is safe to recycle).
fn to_paint(scratch: &mut PaintScratch, brush: &Brush, geometry: &Geometry) -> PaintType {
    match brush {
        Brush::SolidColor(color) => PaintType::Solid(to_peniko_color(color)),
        Brush::LinearGradient { start, end, stops } => {
            let size = geometry_size(geometry);
            let x0 = start.0 * size.width;
            let y0 = start.1 * size.height;
            let x1 = end.0 * size.width;
            let y1 = end.1 * size.height;
            let peniko_stops = &mut scratch.gradient_stops;
            peniko_stops.clear();
            peniko_stops.extend(stops.iter().map(|s| (s.offset, to_peniko_color(&s.color))));
            let gradient =
                Gradient::new_linear((x0, y0), (x1, y1)).with_stops(peniko_stops.as_slice());
            PaintType::Gradient(gradient)
        }
    }
}

fn fill_geometry(
    scene: &mut Scene,
    transform: Affine,
    geometry: &Geometry,
    paint: &PaintType,
    scratch: &mut PaintScratch,
) {
    scene.set_transform(transform);
    scene.set_paint(paint.clone());
    scene.set_fill_rule(Fill::NonZero);
    match geometry {
        Geometry::Rect(size) => {
            scene.fill_rect(&Rect::new(0.0, 0.0, size.width, size.height));
        }
        Geometry::RoundedRect { size, radius } => {
            let path = scratch.rounded_rect_path(size.width, size.height, *radius);
            scene.fill_path(path);
        }
        Geometry::Circle { radius } => {
            let path = scratch.circle_path(*radius);
            scene.fill_path(path);
        }
    }
}

fn stroke_geometry(
    scene: &mut Scene,
    transform: Affine,
    geometry: &Geometry,
    paint: &PaintType,
    stroke_width: f64,
    scratch: &mut PaintScratch,
) {
    scene.set_transform(transform);
    scene.set_paint(paint.clone());
    scene.set_stroke(Stroke::new(stroke_width));
    match geometry {
        Geometry::Rect(size) => {
            scene.stroke_rect(&Rect::new(0.0, 0.0, size.width, size.height));
        }
        Geometry::RoundedRect { size, radius } => {
            let path = scratch.rounded_rect_path(size.width, size.height, *radius);
            scene.stroke_path(path);
        }
        Geometry::Circle { radius } => {
            let path = scratch.circle_path(*radius);
            scene.stroke_path(path);
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
        let paint = to_paint(self.scratch, brush, geometry);
        fill_geometry(self.scene, transform, geometry, &paint, self.scratch);
    }

    #[allow(private_interfaces)]
    fn fill_text_layout(&mut self, offset: Offset, layout: &Arc<TextLayoutData>) {
        let layout: &TextLayoutData = layout;
        let transform = self.current_transform() * Affine::translate((offset.x, offset.y));

        // Cull to the visible line range (conservative — see `text_culling`):
        // a layout taller than the active clip only encodes the runs whose
        // lines intersect it (+ slop). Runs are grouped by ascending
        // `line_index`, so binary-search the first candidate run and stop at
        // the first line past the range.
        let (first_line, end_line) = match self.clip_mirror.current() {
            Some(clip) => match local_clip_y(transform, clip) {
                Some(range) => visible_line_range(&layout.line_infos, Some(range)),
                None => (0, usize::MAX),
            },
            None => (0, usize::MAX),
        };
        let first_run = layout.runs.partition_point(|r| r.line_index < first_line);

        for run in &layout.runs[first_run..] {
            if run.line_index >= end_line {
                break;
            }
            let brush_color =
                PenikoColor::from_rgba8(run.brush[0], run.brush[1], run.brush[2], run.brush[3]);
            // Text color comes from the scene's current paint.
            self.scene.set_transform(transform);
            self.scene.set_paint(PaintType::Solid(brush_color));
            self.scene.set_fill_rule(Fill::NonZero);

            let scene = &mut self.scene;
            let resources = &mut self.resources;
            let builder = scene.glyph_run(resources, &run.font);
            builder
                .font_size(run.font_size)
                .normalized_coords(&run.normalized_coords)
                // Cache rasterized glyphs in vello's persistent `GlyphAtlas`
                // (lives on `Resources`, survives `scene.reset()`). Steady
                // state replaces per-frame bezier→strip flattening with a
                // single textured-quad push per glyph. Rotated/skewed text
                // auto-falls-back to the uncached outline path.
                .atlas_cache(true)
                .fill_glyphs(run.glyphs.iter().map(|g| Glyph {
                    id: g.id,
                    x: g.x,
                    y: g.y,
                }));

            if run.underline
                && let Some(first) = run.glyphs.first()
            {
                let last_x = run
                    .glyphs
                    .last()
                    .map(|g| g.x + g.advance)
                    .unwrap_or(first.x + first.advance);
                let underline_y = first.y + run.font_size * 0.15;
                self.scene.set_stroke(Stroke::new(1.0));
                self.scene.stroke_path(
                    &Line::new(
                        (first.x as f64, underline_y as f64),
                        (last_x as f64, underline_y as f64),
                    )
                    .to_path(TOLERANCE),
                );
            }
        }
    }

    fn draw_image(&mut self, resource_id: ImageResourceId, natural_size: Size, transform: Affine) {
        let Some(&image_id) = self.image_uploads.get(&resource_id) else {
            return;
        };
        let transform = self.current_transform() * transform;
        let image_brush = Image {
            image: ImageSource::opaque_id(image_id),
            sampler: Default::default(),
        };
        self.scene.set_transform(transform);
        self.scene.set_paint(PaintType::Image(image_brush));
        self.scene.set_fill_rule(Fill::NonZero);
        self.scene.fill_rect(&Rect::new(
            0.0,
            0.0,
            natural_size.width,
            natural_size.height,
        ));
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
        let paint = PaintType::Solid(peniko_color);
        stroke_geometry(
            self.scene,
            transform,
            geometry,
            &paint,
            stroke_width,
            self.scratch,
        );
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
            * Affine::translate((offset.x + shadow_offset.0, offset.y + shadow_offset.1));
        let rect = Rect::new(0.0, 0.0, size.width, size.height);
        // `fill_blurred_rounded_rect` uses the current (solid) paint.
        self.scene.set_transform(transform);
        self.scene.set_paint(PaintType::Solid(peniko_color));
        self.scene
            .fill_blurred_rounded_rect(&rect, border_radius as f32, blur as f32, false);
    }

    fn push_clip(&mut self, offset: Offset, size: Size) {
        let transform = self.current_transform() * Affine::translate((offset.x, offset.y));
        let clip = self.scratch.rect_path(size.width, size.height);
        self.scene.set_transform(transform);
        self.scene.push_layer(Some(clip), None, None, None, None);
        self.clip_mirror
            .push_local(transform, Rect::new(0.0, 0.0, size.width, size.height));
    }

    fn push_clip_geometry(&mut self, offset: Offset, geometry: &Geometry) {
        let transform = self.current_transform() * Affine::translate((offset.x, offset.y));
        let clip = match geometry {
            Geometry::Rect(size) => self.scratch.rect_path(size.width, size.height),
            Geometry::RoundedRect { size, radius } => {
                self.scratch
                    .rounded_rect_path(size.width, size.height, *radius)
            }
            Geometry::Circle { radius } => self.scratch.circle_path(*radius),
        };
        self.scene.set_transform(transform);
        self.scene.push_layer(Some(clip), None, None, None, None);
        // Conservative local AABB of the clip geometry (rounded rects and
        // circles use their bounding box).
        let local_aabb = match geometry {
            Geometry::Rect(size) => Rect::new(0.0, 0.0, size.width, size.height),
            Geometry::RoundedRect { size, .. } => Rect::new(0.0, 0.0, size.width, size.height),
            Geometry::Circle { radius } => Rect::new(-radius, -radius, *radius, *radius),
        };
        self.clip_mirror.push_local(transform, local_aabb);
    }

    fn pop_clip(&mut self) {
        self.scene.pop_layer();
        self.clip_mirror.pop();
    }

    fn push_opacity(&mut self, opacity: f32) {
        // Push a layer with reduced alpha and no clip (infinite extent).
        let opacity = opacity.clamp(0.0, 1.0);
        self.scene
            .push_layer(None, Some(BlendMode::default()), Some(opacity), None, None);
    }

    fn pop_opacity(&mut self) {
        self.scene.pop_layer();
    }

    fn push_transform(&mut self, transform: Affine) {
        // Vello Hybrid layers do not transform their content (only their clip),
        // so we compose the affine onto an internal stack and bake it into each
        // subsequent draw call's own transform via `set_transform`. No
        // push_layer is needed for a pure affine.
        let next = self.current_transform() * transform;
        self.transform_stack.push(next);
    }

    fn pop_transform(&mut self) {
        self.transform_stack.pop();
    }

    fn notify_node_entry(&mut self, _id: ElementNodeId, absolute: Affine, _size: Size) {
        // Compose the node's absolute with the root transform (bottom of
        // the stack — the dpr scale) and push without composing with the
        // current top. This matches today's behavior: a node draws at
        // `dpr * node_logical_absolute`, regardless of how deep it is in
        // the tree. The stack always has at least one entry (the root
        // transform seeded in `VelloPaintContext::new`).
        let root_t = self.transform_stack[0];
        self.transform_stack.push(root_t * absolute);
    }

    fn notify_node_exit(&mut self) {
        self.transform_stack.pop();
    }
}

/// Convert a tur [`Color`] into vello's `peniko::Color`.
pub(super) fn to_peniko_color(color: &Color) -> PenikoColor {
    PenikoColor::from_rgba8(color.r(), color.g(), color.b(), color.a())
}
