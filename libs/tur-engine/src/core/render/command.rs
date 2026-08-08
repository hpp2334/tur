//! Render commands — the worker→main paint vocabulary.
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
// worker→main channel contract. If these fail, the new field's type isn't
// Send and needs wrapping (typically `Arc<T>` or a custom `Send` wrapper).
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<CanvasOp>();
    assert_send::<RenderCommand>();
};
