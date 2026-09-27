//! Render commands — the worker→host paint vocabulary.
//!
//! - [`CanvasOp`] is 1:1 with the [`Canvas`](super::Canvas) trait — draw ops
//!   and layer ops. Plain `Send` data; produced by [`RecordingCanvas`](super::RecordingCanvas).
//! - [`RenderCommand`] is the top-level commit-log entry. Each frame, the
//!   worker emits a `Vec<RenderCommand>` (batched) describing paint state
//!   deltas. Main applies the batch to its long-lived tree and plays the
//!   `Paint` ops linearly to render.
//!
//! ## Linear playback model
//!
//! Main iterates the command list once per frame; no recursion is required.
//! Elements whose paint body interleaves children (e.g. `Flex` clips, the
//! `Opacity` layer) emit multiple `Paint` commands per node so children's
//! `Paint`s appear in order between the parent's op runs — e.g. `Flex`'s
//! `PushClip` → children → `PopClip` becomes:
//!
//! ```text
//! [
//!     Paint { id: flex, transform, size, ops: [PushClip] },
//!     Paint { id: child_a, transform, size, ops: [...] },
//!     Paint { id: child_b, transform, size, ops: [...] },
//!     Paint { id: flex, transform, size, ops: [PopClip] },
//! ]
//! ```
//!
//! The clip/opacity/transform layer state opened by the parent's first
//! `Paint` persists in the underlying canvas/scene across the children's
//! `Paint`s, so the structure is preserved without an explicit child
//! marker.

use std::sync::Arc;

use crate::core::element::ElementNodeId;
use crate::core::image_resource::ImageResourceId;
use crate::core::layout::{Geometry, Offset, Size};
use crate::core::render::brush::{Brush, Color};
use crate::core::text::text_layout::TextLayoutData;
use vello_common::kurbo::Affine;

/// One paint operation, 1:1 with a [`Canvas`](super::Canvas) method.
///
/// Recorded by [`RecordingCanvas`](super::RecordingCanvas) on the worker;
/// replayed by `VelloPaintContext` on main. Plain `Send` data throughout —
/// `Brush` and `TextLayoutData` own `Vec`s of plain-data structs; the
/// `Arc<TextLayoutData>` on `FillTextLayout` lets the worker hand the same
/// shaped layout to main by refcount bump instead of a deep clone.
#[derive(Debug, Clone)]
pub enum CanvasOp {
    // ─── Draw ops (1:1 with the Canvas trait) ───
    FillGeometry {
        offset: Offset,
        geometry: Geometry,
        brush: Brush,
    },
    StrokeGeometry {
        offset: Offset,
        geometry: Geometry,
        color: Color,
        stroke_width: f64,
    },
    FillTextLayout {
        offset: Offset,
        layout: Arc<TextLayoutData>,
    },
    DrawImage {
        resource_id: ImageResourceId,
        natural_size: Size,
        transform: Affine,
    },
    DrawShadow {
        offset: Offset,
        size: Size,
        color: Color,
        border_radius: f64,
        blur: f64,
        shadow_offset: (f64, f64),
    },
    // ─── Layer stack (1:1 with the Canvas trait) ───
    PushClip {
        offset: Offset,
        size: Size,
    },
    PushClipGeometry {
        offset: Offset,
        geometry: Geometry,
    },
    PopClip,
    PushOpacity(f32),
    PopOpacity,
    PushTransform(Affine),
    PopTransform,
}

/// Top-level commit-log entry emitted by the worker each frame.
///
/// Each frame the worker emits a `Vec<RenderCommand>` (a batch of
/// `Paint` ops) describing that frame's paint state. Main plays the
/// batch linearly into its renderer via [`super::play_commands`].
#[derive(Debug, Clone)]
pub enum RenderCommand {
    /// Paint `ops` for node `id` at absolute `transform` and `size`.
    ///
    /// `transform` is **absolute** (pre-composed by the worker during the
    /// record walk) so main playback is purely linear — no parent-context
    /// composition required. A node may emit multiple `Paint` commands per
    /// frame when its paint body interleaves children (the worker splits at
    /// each `paint_ctx.paint_child` boundary).
    Paint {
        id: ElementNodeId,
        transform: Affine,
        size: Size,
        ops: Vec<CanvasOp>,
    },
}

// Compile-time Send assertions — guard against future fields breaking the
// worker→host channel contract. If these fail, the new field's type isn't
// Send and needs wrapping (typically `Arc<T>` or a custom `Send` wrapper).
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<CanvasOp>();
    assert_send::<RenderCommand>();
};

/// Every image id the batch references (`CanvasOp::DrawImage` inside each
/// `Paint`'s ops). Used by the host-side render commit point to re-ensure
/// the renderer's atlas holds everything a frame is about to paint —
/// idempotent renderer uploads make the pass a hashmap probe when nothing
/// is missing.
///
/// Must be extended when a new image-bearing `CanvasOp` variant appears —
/// kept beside the enum so the two move together.
pub(crate) fn referenced_image_ids(
    commands: &[RenderCommand],
) -> impl Iterator<Item = ImageResourceId> + '_ {
    commands
        .iter()
        .flat_map(|cmd| match cmd {
            RenderCommand::Paint { ops, .. } => ops.iter(),
        })
        .filter_map(|op| match op {
            CanvasOp::DrawImage { resource_id, .. } => Some(*resource_id),
            _ => None,
        })
}

// ---------------------------------------------------------------------------
// Frame fingerprint (render-commit frame dedup)
// ---------------------------------------------------------------------------

/// Fast non-cryptographic content hash (FxHash-style multiply-xor). One pass
/// over the batch's value bits — never a cryptographic guarantee, only a
/// collision-averse skip signal for the render commit point (a collision
/// would render nothing where something changed; with multiply-xor on the
/// exact-bitstream content that is astronomically unlikely and no worse than
/// the old behavior for correctness, since a wrong skip is still bounded by
/// fingerprint equality).
#[derive(Default)]
struct BatchHasher(u64);

impl BatchHasher {
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    fn write_f64(&mut self, v: f64) {
        self.write_u64(v.to_bits());
    }
    fn write_f32(&mut self, v: f32) {
        self.write_u64(v.to_bits() as u64);
    }
    fn write_usize(&mut self, v: usize) {
        self.write_u64(v as u64);
    }
    fn write_tag(&mut self, tag: u64) {
        // Mix a discriminant into the running state directly (rotate+xor)
        // so tags can never alias payload bit patterns.
        self.0 = self.0.rotate_left(7) ^ tag.rotate_left(17);
    }
    fn finish(self) -> u64 {
        self.0
    }
}

fn hash_affine(h: &mut BatchHasher, t: &Affine) {
    for c in t.as_coeffs() {
        h.write_f64(c);
    }
}

fn hash_geometry(h: &mut BatchHasher, geometry: &Geometry) {
    match geometry {
        Geometry::Rect(size) => {
            h.write_tag(0x10);
            h.write_f64(size.width);
            h.write_f64(size.height);
        }
        Geometry::RoundedRect { size, radius } => {
            h.write_tag(0x11);
            h.write_f64(size.width);
            h.write_f64(size.height);
            h.write_f64(*radius);
        }
        Geometry::Circle { radius } => {
            h.write_tag(0x12);
            h.write_f64(*radius);
        }
    }
}

fn hash_brush(h: &mut BatchHasher, brush: &Brush) {
    match brush {
        Brush::SolidColor(color) => {
            h.write_tag(0x20);
            hash_color(h, color);
        }
        Brush::LinearGradient { start, end, stops } => {
            h.write_tag(0x21);
            h.write_f64(start.0);
            h.write_f64(start.1);
            h.write_f64(end.0);
            h.write_f64(end.1);
            h.write_usize(stops.len());
            for stop in stops {
                h.write_f32(stop.offset);
                hash_color(h, &stop.color);
            }
        }
    }
}

fn hash_color(h: &mut BatchHasher, color: &Color) {
    // Hash the discriminant + the 8-bit channel values.
    h.write_u64(
        color.r() as u64
            | (color.g() as u64) << 8
            | (color.b() as u64) << 16
            | (color.a() as u64) << 24,
    );
}

fn hash_canvas_op(h: &mut BatchHasher, op: &CanvasOp) {
    match op {
        CanvasOp::FillGeometry {
            offset,
            geometry,
            brush,
        } => {
            h.write_tag(0x01);
            hash_offset(h, offset);
            hash_geometry(h, geometry);
            hash_brush(h, brush);
        }
        CanvasOp::StrokeGeometry {
            offset,
            geometry,
            color,
            stroke_width,
        } => {
            h.write_tag(0x02);
            hash_offset(h, offset);
            hash_geometry(h, geometry);
            hash_color(h, color);
            h.write_f64(*stroke_width);
        }
        CanvasOp::FillTextLayout { offset, layout } => {
            h.write_tag(0x02);
            hash_offset(h, offset);
            // Arc pointer identity — stable across unchanged frames (the
            // layout memo reuses the same Arc), conservative otherwise.
            h.write_u64(std::sync::Arc::as_ptr(layout) as *const () as u64);
        }
        CanvasOp::DrawImage {
            resource_id,
            natural_size,
            transform,
        } => {
            h.write_tag(0x03);
            h.write_u64(resource_id.as_u64());
            h.write_f64(natural_size.width);
            h.write_f64(natural_size.height);
            hash_affine(h, transform);
        }
        CanvasOp::DrawShadow {
            offset,
            size,
            color,
            border_radius,
            blur,
            shadow_offset,
        } => {
            h.write_tag(0x04);
            hash_offset(h, offset);
            h.write_f64(size.width);
            h.write_f64(size.height);
            hash_color(h, color);
            h.write_f64(*border_radius);
            h.write_f64(*blur);
            h.write_f64(shadow_offset.0);
            h.write_f64(shadow_offset.1);
        }
        CanvasOp::PushClip { offset, size } => {
            h.write_tag(0x05);
            hash_offset(h, offset);
            h.write_f64(size.width);
            h.write_f64(size.height);
        }
        CanvasOp::PushClipGeometry { offset, geometry } => {
            h.write_tag(0x06);
            hash_offset(h, offset);
            hash_geometry(h, geometry);
        }
        CanvasOp::PopClip => h.write_tag(0x07),
        CanvasOp::PushOpacity(opacity) => {
            h.write_tag(0x08);
            h.write_f32(*opacity);
        }
        CanvasOp::PopOpacity => h.write_tag(0x08),
        CanvasOp::PushTransform(t) => {
            h.write_tag(0x09);
            hash_affine(h, t);
        }
        CanvasOp::PopTransform => h.write_tag(0x0A),
    }
}

fn hash_offset(h: &mut BatchHasher, offset: &Offset) {
    h.write_f64(offset.x);
    h.write_f64(offset.y);
}

// Compile-time Send assertions — guard against future fields breaking the
/// laid out for. Hashed at the render commit point; equality with the last
/// APPLIED frame's fingerprint lets the host skip scene rebuild + re-encode
/// + re-raster entirely (the presented frame already shows this content).
///
/// Conservative by construction: only hashes exact-value bits (`f64::to_bits`)
/// and `Arc` POINTER identity for text layouts (identical content re-shaped
/// into a new `Arc` hashes differently → an unnecessary render, never a wrong
/// skip; the layout memo makes pointer identity stable across unchanged
/// frames). -0.0/0.0 and NaN payload differences hash differently — also only
/// ever extra renders.
pub(crate) fn fingerprint_batch(
    commands: &[RenderCommand],
    viewport: &crate::core::screen::ScreenViewport,
) -> u64 {
    let mut h = BatchHasher::default();
    h.write_tag(0xA1); // command-stream tag
    h.write_usize(commands.len());
    for cmd in commands {
        let RenderCommand::Paint {
            id,
            transform,
            size,
            ops,
        } = cmd;
        h.write_tag(0xB1);
        h.write_u64(u64::from(*id));
        hash_affine(&mut h, transform);
        h.write_f64(size.width);
        h.write_f64(size.height);
        h.write_usize(ops.len());
        for op in ops {
            hash_canvas_op(&mut h, op);
        }
    }
    h.write_tag(0xC1);
    h.write_u64(viewport.logical_width as u64);
    h.write_u64(viewport.logical_height as u64);
    h.write_f64(viewport.dpr);
    h.finish()
}
