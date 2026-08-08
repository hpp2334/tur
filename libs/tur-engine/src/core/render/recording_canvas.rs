//! A [`Canvas`] impl that records every method call plus per-node boundary
//! markers, producing a flat op stream that the record pass post-processes
//! into `Vec<RenderCommand>` (one or more `Paint` commands per node).
//!
//! ## Why node-boundary markers
//!
//! Elements whose paint body interleaves children (e.g. `Flex`'s
//! `PushClip` → children → `PopClip`) need their op stream split at each
//! `paint_ctx.paint_child` boundary so each child's `Paint` appears in
//! order between the parent's op runs. The paint walk calls
//! [`Canvas::notify_node_entry`] / [`Canvas::notify_node_exit`] around
//! every node (including recursed children), so recording those markers
//! captures the interleaving exactly. See [`RecordingOp`] and
//! [`RecordingCanvas::into_render_commands`].

use crate::core::element::ElementNodeId;
use crate::core::image_resource::ImageResourceId;
use crate::core::layout::{Geometry, Offset, Size};
use crate::core::render::Canvas;
use crate::core::render::brush::{Brush, Color};
use crate::core::render::command::{CanvasOp, RenderCommand};
use crate::core::text::text_layout::TextLayoutData;
use std::fmt;
use std::sync::Arc;
use vello_common::kurbo::{Affine, Point, Rect};

/// Internal recording entry — either a recorded canvas op or a per-node
/// boundary marker placed by the paint walk's `notify_node_entry` /
/// `notify_node_exit` callbacks.
///
/// Not part of the public wire format — only lives between
/// `RecordingCanvas` and the post-processing step that produces
/// `Vec<RenderCommand>`.
enum RecordingOp {
    Canvas(CanvasOp),
    NodeStart {
        id: ElementNodeId,
        transform: Affine,
        size: Size,
    },
    NodeEnd,
}

/// Recording canvas — records every `Canvas` method call (wrapped as
/// `RecordingOp::Canvas`) plus `NodeStart`/`NodeEnd` markers from the
/// paint walk. Owns its op vec so the record pass can extract it via
/// [`into_render_commands`](Self::into_render_commands) without lifetime
/// gymnastics.
///
/// In addition to recording, it mirrors the transform + clip stacks the
/// playback canvas would maintain, so the record pass can answer
/// [`Canvas::current_clip_rect`] and let `NodeTreeData::paint_element`
/// **cull fully-clipped subtrees before recording them** (skips the element
/// paint body + the whole subtree — the high-value win for long scrollable
/// lists where most children are off-screen).
pub struct RecordingCanvas {
    ops: Vec<RecordingOp>,
    /// Mirror of the playback transform stack, in **logical** space (root =
    /// `IDENTITY`). `notify_node_entry` pushes the node's `absolute`
    /// affine; `push_transform` composes. Used only to compute clip rects.
    transform_stack: Vec<Affine>,
    /// Conservative logical-space AABBs of active clips, innermost last.
    /// Empty when no clip is active (→ [`Canvas::current_clip_rect`] returns
    /// `None` → no culling). A viewport seed can populate it at construction
    /// (see [`RecordingCanvas::new_with_viewport`]).
    clip_stack: Vec<Rect>,
}

impl fmt::Debug for RecordingCanvas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordingCanvas")
            .field("ops_len", &self.ops.len())
            .finish_non_exhaustive()
    }
}

impl Default for RecordingCanvas {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordingCanvas {
    pub fn new() -> Self {
        Self {
            ops: Vec::new(),
            transform_stack: vec![Affine::IDENTITY],
            clip_stack: Vec::new(),
        }
    }

    /// Construct with a viewport seed (logical-space rect). The viewport is
    /// pushed as the bottom-of-stack clip so content outside the screen is
    /// culled too. Used by the production record path; tests that want the
    /// legacy no-culling behavior keep using [`RecordingCanvas::new`].
    pub fn new_with_viewport(viewport: Rect) -> Self {
        Self {
            ops: Vec::new(),
            transform_stack: vec![Affine::IDENTITY],
            clip_stack: vec![viewport],
        }
    }

    /// Current accumulated transform (logical space). Always defined because
    /// the stack is seeded with `IDENTITY`.
    fn current_transform(&self) -> Affine {
        self.transform_stack
            .last()
            .copied()
            .unwrap_or(Affine::IDENTITY)
    }

    /// Push a clip defined in **local** space (relative to the current
    /// transform). Transforms the local rect's 4 corners into scene space,
    /// takes the AABB, and intersects it with the current innermost clip (or
    /// keeps it as-is when no clip is active yet). Conservative: rotated
    /// clips report their AABB, so a visible node is never wrongly culled.
    fn push_clip_local(&mut self, local_rect: Rect) {
        let t = self.current_transform();
        let corners = [
            t * Point::new(local_rect.x0, local_rect.y0),
            t * Point::new(local_rect.x1, local_rect.y0),
            t * Point::new(local_rect.x0, local_rect.y1),
            t * Point::new(local_rect.x1, local_rect.y1),
        ];
        let xs = [corners[0].x, corners[1].x, corners[2].x, corners[3].x];
        let ys = [corners[0].y, corners[1].y, corners[2].y, corners[3].y];
        let x0 = xs.iter().copied().fold(f64::INFINITY, f64::min);
        let y0 = ys.iter().copied().fold(f64::INFINITY, f64::min);
        let x1 = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let y1 = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let scene_rect = Rect::new(x0, y0, x1, y1);
        let intersected = self
            .clip_stack
            .last()
            .map(|top| top.intersect(scene_rect))
            .unwrap_or(scene_rect);
        self.clip_stack.push(intersected);
    }

    /// Post-process the recorded op stream into a flat `Vec<RenderCommand>`
    /// (one or more `Paint` commands per node, in playback order).
    ///
    /// Algorithm: stack-based walk over the recorded ops. Each `NodeStart`
    /// closes the parent's current segment (emits a `Paint` if non-empty),
    /// then pushes a new frame. Each `Canvas` op appends to the top frame's
    /// current segment. Each `NodeEnd` pops and emits a final `Paint` for
    /// the node's last segment if non-empty.
    ///
    /// This naturally handles interleaved children: a parent like `Flex`
    /// that emits `[PushClip]` then recurses into children (which push
    /// their own `NodeStart`/`NodeEnd`) then emits `[PopClip]` becomes
    /// three `Paint` commands — `[PushClip]` before the children,
    /// children's `Paint`s in order, then `[PopClip]` after.
    pub fn into_render_commands(self) -> Vec<RenderCommand> {
        let mut commands: Vec<RenderCommand> = Vec::new();
        // Stack of (id, transform, size, current_segment_ops). The bottom
        // of the stack is the outermost node being painted.
        let mut stack: Vec<(ElementNodeId, Affine, Size, Vec<CanvasOp>)> = Vec::new();

        for op in self.ops {
            match op {
                RecordingOp::Canvas(canvas_op) => {
                    // Append to the top frame's current segment. If there's
                    // no frame (canvas op outside any node — shouldn't happen
                    // with the current paint walk, but defensive), drop it.
                    if let Some((_, _, _, segment)) = stack.last_mut() {
                        segment.push(canvas_op);
                    }
                }
                RecordingOp::NodeStart {
                    id,
                    transform,
                    size,
                } => {
                    // Close the parent's current segment: if non-empty,
                    // emit a Paint for the parent's op run that just ended,
                    // then reset its segment to start a fresh run after the
                    // child returns.
                    if let Some((pid, ptransform, psize, psegment)) = stack.last_mut()
                        && !psegment.is_empty()
                    {
                        commands.push(RenderCommand::Paint {
                            id: *pid,
                            transform: *ptransform,
                            size: *psize,
                            ops: std::mem::take(psegment),
                        });
                    }
                    stack.push((id, transform, size, Vec::new()));
                }
                RecordingOp::NodeEnd => {
                    if let Some((id, transform, size, segment)) = stack.pop()
                        && !segment.is_empty()
                    {
                        commands.push(RenderCommand::Paint {
                            id,
                            transform,
                            size,
                            ops: segment,
                        });
                    }
                }
            }
        }

        commands
    }
}

impl Canvas for RecordingCanvas {
    fn fill_geometry(&mut self, offset: Offset, geometry: &Geometry, brush: &Brush) {
        self.ops.push(RecordingOp::Canvas(CanvasOp::FillGeometry {
            offset,
            geometry: *geometry,
            brush: brush.clone(),
        }));
    }

    fn stroke_geometry(
        &mut self,
        offset: Offset,
        geometry: &Geometry,
        color: &Color,
        stroke_width: f64,
    ) {
        self.ops.push(RecordingOp::Canvas(CanvasOp::StrokeGeometry {
            offset,
            geometry: *geometry,
            color: *color,
            stroke_width,
        }));
    }

    fn fill_text_layout(&mut self, offset: Offset, layout: &Arc<TextLayoutData>) {
        self.ops.push(RecordingOp::Canvas(CanvasOp::FillTextLayout {
            offset,
            layout: Arc::clone(layout),
        }));
    }

    fn draw_image(&mut self, resource_id: ImageResourceId, natural_size: Size, transform: Affine) {
        self.ops.push(RecordingOp::Canvas(CanvasOp::DrawImage {
            resource_id,
            natural_size,
            transform,
        }));
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
        self.ops.push(RecordingOp::Canvas(CanvasOp::DrawShadow {
            offset,
            size,
            color: *color,
            border_radius,
            blur,
            shadow_offset,
        }));
    }

    fn push_clip(&mut self, offset: Offset, size: Size) {
        self.ops
            .push(RecordingOp::Canvas(CanvasOp::PushClip { offset, size }));
        self.push_clip_local(Rect::new(
            offset.x,
            offset.y,
            offset.x + size.width,
            offset.y + size.height,
        ));
    }

    fn push_clip_geometry(&mut self, offset: Offset, geometry: &Geometry) {
        self.ops
            .push(RecordingOp::Canvas(CanvasOp::PushClipGeometry {
                offset,
                geometry: *geometry,
            }));
        // Conservative local AABB of the clip shape (rounded/circle → its
        // bounding box) — a rotated clip's true shape isn't axis-aligned,
        // but its AABB is a superset, so culling never drops a visible node.
        let local = match geometry {
            Geometry::Rect(size) => Rect::new(0.0, 0.0, size.width, size.height),
            Geometry::RoundedRect { size, .. } => Rect::new(0.0, 0.0, size.width, size.height),
            Geometry::Circle { radius } => {
                let d = radius * 2.0;
                Rect::new(0.0, 0.0, d, d)
            }
        }
        .with_origin((offset.x, offset.y));
        self.push_clip_local(local);
    }

    fn pop_clip(&mut self) {
        self.ops.push(RecordingOp::Canvas(CanvasOp::PopClip));
        // The viewport seed (if any) is the bottom of the stack and is never
        // popped by element paint bodies, so only pop when there's more than
        // the seed. Defensive: never drain below empty.
        self.clip_stack.pop();
    }

    fn push_opacity(&mut self, opacity: f32) {
        self.ops
            .push(RecordingOp::Canvas(CanvasOp::PushOpacity(opacity)));
    }

    fn pop_opacity(&mut self) {
        self.ops.push(RecordingOp::Canvas(CanvasOp::PopOpacity));
    }

    fn push_transform(&mut self, transform: Affine) {
        self.ops
            .push(RecordingOp::Canvas(CanvasOp::PushTransform(transform)));
        // Mirror VelloPaintContext: compose onto the current top.
        let next = self.current_transform() * transform;
        self.transform_stack.push(next);
    }

    fn pop_transform(&mut self) {
        self.ops.push(RecordingOp::Canvas(CanvasOp::PopTransform));
        self.transform_stack.pop();
    }

    fn notify_node_entry(&mut self, id: ElementNodeId, transform: Affine, size: Size) {
        self.ops.push(RecordingOp::NodeStart {
            id,
            transform,
            size,
        });
        // Mirror VelloPaintContext::notify_node_entry: each node draws at
        // `root * absolute`. The record-pass root is `IDENTITY` (logical
        // space), so this is just the node's `absolute` affine.
        self.transform_stack.push(transform);
    }

    fn notify_node_exit(&mut self) {
        self.ops.push(RecordingOp::NodeEnd);
        self.transform_stack.pop();
    }

    fn current_clip_rect(&self) -> Option<Rect> {
        self.clip_stack.last().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::{Geometry, Size};
    use crate::core::render::brush::{Brush, Color};

    fn nid(n: u64) -> ElementNodeId {
        ElementNodeId::new(n)
    }

    /// A leaf node that draws a single fill produces exactly one Paint
    /// command with that op.
    #[test]
    fn single_leaf_node_one_paint() {
        let mut canvas = RecordingCanvas::new();
        canvas.notify_node_entry(nid(1), Affine::IDENTITY, Size::new(10.0, 10.0));
        canvas.fill_geometry(
            Offset::ZERO,
            &Geometry::Rect(Size::new(10.0, 10.0)),
            &Brush::SolidColor(Color::rgb(255, 0, 0)),
        );
        canvas.notify_node_exit();

        let cmds = canvas.into_render_commands();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            RenderCommand::Paint {
                id: paint_id, ops, ..
            } => {
                assert_eq!(*paint_id, nid(1));
                assert_eq!(ops.len(), 1); // just the fill_geometry
            }
            other => panic!("expected Paint, got {other:?}"),
        }
    }

    /// A node that interleaves children between two op runs (the Flex
    /// `[PushClip] → children → [PopClip]` pattern) splits into multiple
    /// Paint commands for the parent.
    #[test]
    fn parent_splits_around_children() {
        let mut canvas = RecordingCanvas::new();
        // Parent entry
        canvas.notify_node_entry(nid(1), Affine::IDENTITY, Size::new(100.0, 100.0));
        canvas.push_clip(Offset::ZERO, Size::new(100.0, 100.0));
        // Child entry (parent's segment becomes [PushClip])
        canvas.notify_node_entry(nid(2), Affine::IDENTITY, Size::new(50.0, 50.0));
        canvas.fill_geometry(
            Offset::ZERO,
            &Geometry::Rect(Size::new(50.0, 50.0)),
            &Brush::SolidColor(Color::rgb(0, 255, 0)),
        );
        canvas.notify_node_exit();
        // Back to parent — PopClip
        canvas.pop_clip();
        canvas.notify_node_exit();

        let cmds = canvas.into_render_commands();
        // Expect 3 Paint commands: parent[PushClip], child[FillGeometry], parent[PopClip]
        assert_eq!(cmds.len(), 3, "got {cmds:?}");

        let parent_ids: Vec<ElementNodeId> = cmds
            .iter()
            .filter_map(|c| match c {
                RenderCommand::Paint { id, .. } if *id == nid(1) => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(parent_ids.len(), 2, "parent should have 2 Paint segments");

        let child_ids: Vec<ElementNodeId> = cmds
            .iter()
            .filter_map(|c| match c {
                RenderCommand::Paint { id, .. } if *id == nid(2) => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(child_ids.len(), 1, "child should have 1 Paint segment");

        // Verify ordering: parent_segment_1, child, parent_segment_2
        match (&cmds[0], &cmds[1], &cmds[2]) {
            (
                RenderCommand::Paint {
                    id: p1, ops: ops1, ..
                },
                RenderCommand::Paint {
                    id: c, ops: cops, ..
                },
                RenderCommand::Paint {
                    id: p2, ops: ops2, ..
                },
            ) => {
                assert_eq!(*p1, nid(1));
                assert_eq!(*c, nid(2));
                assert_eq!(*p2, nid(1));
                assert_eq!(ops1.len(), 1); // PushClip
                assert!(matches!(ops1[0], CanvasOp::PushClip { .. }));
                assert_eq!(cops.len(), 1); // FillGeometry
                assert!(matches!(cops[0], CanvasOp::FillGeometry { .. }));
                assert_eq!(ops2.len(), 1); // PopClip
                assert!(matches!(ops2[0], CanvasOp::PopClip));
            }
            _ => panic!("unexpected command pattern"),
        }
    }

    /// A pure pass-through node (no draw ops of its own — e.g. `Stack`)
    /// produces zero Paint commands even though it has children.
    #[test]
    fn passthrough_node_emits_no_paint() {
        let mut canvas = RecordingCanvas::new();
        canvas.notify_node_entry(nid(1), Affine::IDENTITY, Size::new(100.0, 100.0));
        // No draws — immediately descend to child
        canvas.notify_node_entry(nid(2), Affine::IDENTITY, Size::new(50.0, 50.0));
        canvas.fill_geometry(
            Offset::ZERO,
            &Geometry::Rect(Size::new(50.0, 50.0)),
            &Brush::SolidColor(Color::rgb(0, 0, 255)),
        );
        canvas.notify_node_exit();
        canvas.notify_node_exit();

        let cmds = canvas.into_render_commands();
        // Parent contributes no Paint (empty segment); child contributes one.
        assert_eq!(cmds.len(), 1, "got {cmds:?}");
        match &cmds[0] {
            RenderCommand::Paint { id: paint_id, .. } => assert_eq!(*paint_id, nid(2)),
            other => panic!("expected Paint, got {other:?}"),
        }
    }
}
