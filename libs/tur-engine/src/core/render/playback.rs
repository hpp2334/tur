//! Linear playback of a `Vec<RenderCommand>` into any [`Canvas`].
//!
//! Used by the new render path (Phase 3): the worker-side record pass
//! captures a flat command batch (see [`RecordingCanvas`](super::RecordingCanvas)),
//! and main plays it back here into its own canvas/scene. There is no
//! recursion and no parent-chain lookup — each [`RenderCommand::Paint`]
//! carries its own absolute transform, so playback is purely linear.
//!
//! ## Layer-state continuity
//!
//! Elements whose paint body interleaves children (e.g. `Flex` clips, the
//! `Opacity` layer) split into multiple `Paint` commands per node so
//! children's `Paint`s appear in order between the parent's op runs. The
//! clip/opacity/transform layer state opened by a parent's first `Paint`
//! persists in the underlying canvas across the children's `Paint`s, so the
//! structure is preserved without an explicit child marker — playback just
//! walks the list top-to-bottom.
//!
//! ## Canvas hooks
//!
//! Each `Paint` is wrapped in a
//! [`Canvas::notify_node_entry`](super::Canvas::notify_node_entry) /
//! [`Canvas::notify_node_exit`](super::Canvas::notify_node_exit) pair, so
//! canvases that maintain their own transform stack (e.g.
//! [`VelloPaintContext`](crate::renderer::vello::VelloPaintContext)) compose
//! the per-node absolute affine the same way the live paint walk does.
//! `RecordingCanvas` ignores the hooks (already captured at record time);
//! `NullCanvas` no-ops them.

use crate::core::render::Canvas;
use crate::core::render::CanvasOp;
use crate::core::render::RenderCommand;

/// Play `commands` into `canvas` in order.
///
/// `Paint` → `notify_node_entry` + ops + `notify_node_exit`.
pub fn play_commands(canvas: &mut dyn Canvas, commands: &[RenderCommand]) {
    for cmd in commands {
        match cmd {
            RenderCommand::Paint {
                id,
                transform,
                size,
                ops,
            } => {
                canvas.notify_node_entry(*id, *transform, *size);
                for op in ops {
                    dispatch_canvas_op(canvas, op);
                }
                canvas.notify_node_exit();
            }
        }
    }
}

fn dispatch_canvas_op(canvas: &mut dyn Canvas, op: &CanvasOp) {
    match op {
        CanvasOp::FillGeometry {
            offset,
            geometry,
            brush,
        } => canvas.fill_geometry(*offset, geometry, brush),
        CanvasOp::StrokeGeometry {
            offset,
            geometry,
            color,
            stroke_width,
        } => canvas.stroke_geometry(*offset, geometry, color, *stroke_width),
        CanvasOp::FillTextLayout { offset, layout } => {
            canvas.fill_text_layout(*offset, layout);
        }
        CanvasOp::DrawImage {
            resource_id,
            natural_size,
            transform,
        } => canvas.draw_image(*resource_id, *natural_size, *transform),
        CanvasOp::DrawShadow {
            offset,
            size,
            color,
            border_radius,
            blur,
            shadow_offset,
        } => canvas.draw_shadow(*offset, *size, color, *border_radius, *blur, *shadow_offset),
        CanvasOp::PushClip { offset, size } => canvas.push_clip(*offset, *size),
        CanvasOp::PushClipGeometry { offset, geometry } => {
            canvas.push_clip_geometry(*offset, geometry)
        }
        CanvasOp::PopClip => canvas.pop_clip(),
        CanvasOp::PushOpacity(opacity) => canvas.push_opacity(*opacity),
        CanvasOp::PopOpacity => canvas.pop_opacity(),
        CanvasOp::PushTransform(t) => canvas.push_transform(*t),
        CanvasOp::PopTransform => canvas.pop_transform(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::element::ElementNodeId;
    use crate::core::layout::{Geometry, Offset, Size};
    use crate::core::render::RecordingCanvas;
    use crate::core::render::brush::{Brush, Color};
    use std::sync::Arc;
    use vello_common::kurbo::Affine;

    fn nid(n: u64) -> ElementNodeId {
        ElementNodeId::new(n)
    }

    /// A canvas that records every call as a string for assertion-friendly
    /// inspection in playback tests. The transform stack is also tracked
    /// so we can verify `notify_node_entry` / `exit` are paired correctly.
    #[derive(Debug, Default)]
    struct CapturingCanvas {
        calls: Vec<String>,
        transform_depth: usize,
    }

    impl CapturingCanvas {
        fn new() -> Self {
            Self {
                calls: Vec::new(),
                transform_depth: 0,
            }
        }
    }

    impl Canvas for CapturingCanvas {
        fn fill_geometry(&mut self, offset: Offset, geometry: &Geometry, _brush: &Brush) {
            self.calls.push(format!("fill({offset:?}, {geometry:?})"));
        }
        fn stroke_geometry(
            &mut self,
            offset: Offset,
            geometry: &Geometry,
            _color: &Color,
            _w: f64,
        ) {
            self.calls.push(format!("stroke({offset:?}, {geometry:?})"));
        }
        fn fill_text_layout(
            &mut self,
            offset: Offset,
            _layout: &Arc<crate::core::text::text_layout::TextLayoutData>,
        ) {
            self.calls.push(format!("text({offset:?})"));
        }
        fn draw_image(
            &mut self,
            _resource_id: crate::core::image_resource::ImageResourceId,
            _natural_size: Size,
            _transform: Affine,
        ) {
            self.calls.push("image".to_string());
        }
        fn draw_shadow(
            &mut self,
            offset: Offset,
            _size: Size,
            _color: &Color,
            _br: f64,
            _blur: f64,
            _so: (f64, f64),
        ) {
            self.calls.push(format!("shadow({offset:?})"));
        }
        fn push_clip(&mut self, offset: Offset, size: Size) {
            self.calls.push(format!("push_clip({offset:?}, {size:?})"));
        }
        fn push_clip_geometry(&mut self, offset: Offset, geometry: &Geometry) {
            self.calls
                .push(format!("push_clip_geometry({offset:?}, {geometry:?})"));
        }
        fn pop_clip(&mut self) {
            self.calls.push("pop_clip".to_string());
        }
        fn push_opacity(&mut self, opacity: f32) {
            self.calls.push(format!("push_opacity({opacity})"));
        }
        fn pop_opacity(&mut self) {
            self.calls.push("pop_opacity".to_string());
        }
        fn push_transform(&mut self, transform: Affine) {
            self.calls.push(format!("push_transform({transform:?})"));
        }
        fn pop_transform(&mut self) {
            self.calls.push("pop_transform".to_string());
        }
        fn notify_node_entry(&mut self, id: ElementNodeId, _t: Affine, _s: Size) {
            self.calls.push(format!(">> {id}"));
            self.transform_depth += 1;
        }
        fn notify_node_exit(&mut self) {
            self.calls.push("<< node".to_string());
            assert!(self.transform_depth > 0, "unbalanced notify_node_exit");
            self.transform_depth -= 1;
        }
    }

    fn fill_red() -> CanvasOp {
        CanvasOp::FillGeometry {
            offset: Offset::ZERO,
            geometry: Geometry::Rect(Size::new(10.0, 10.0)),
            brush: Brush::SolidColor(Color::rgb(255, 0, 0)),
        }
    }

    /// A single Paint command plays back as `notify_entry → ops → notify_exit`.
    #[test]
    fn single_paint_plays_in_order() {
        let commands = vec![RenderCommand::Paint {
            id: nid(1),
            transform: Affine::IDENTITY,
            size: Size::new(10.0, 10.0),
            ops: vec![fill_red()],
        }];

        let mut canvas = CapturingCanvas::new();
        play_commands(&mut canvas, &commands);

        assert_eq!(canvas.calls.len(), 3);
        assert!(canvas.calls[0].starts_with(">>"));
        assert!(canvas.calls[1].starts_with("fill"));
        assert!(canvas.calls[2].starts_with("<<"));
        assert_eq!(canvas.transform_depth, 0);
    }

    /// A Paint with an interleaved child (parent pushes clip, then a child
    /// Paint runs, then parent pops clip) plays back in the right order —
    /// the layer state is continuous because all ops go into the same canvas.
    #[test]
    fn interleaved_paints_preserve_layer_continuity() {
        let commands = vec![
            RenderCommand::Paint {
                id: nid(1),
                transform: Affine::IDENTITY,
                size: Size::new(100.0, 100.0),
                ops: vec![CanvasOp::PushClip {
                    offset: Offset::ZERO,
                    size: Size::new(100.0, 100.0),
                }],
            },
            RenderCommand::Paint {
                id: nid(2),
                transform: Affine::IDENTITY,
                size: Size::new(50.0, 50.0),
                ops: vec![fill_red()],
            },
            RenderCommand::Paint {
                id: nid(1),
                transform: Affine::IDENTITY,
                size: Size::new(100.0, 100.0),
                ops: vec![CanvasOp::PopClip],
            },
        ];

        let mut canvas = CapturingCanvas::new();
        play_commands(&mut canvas, &commands);

        // Expected sequence: parent entry, push_clip, parent exit,
        // child entry, fill, child exit, parent entry (2nd segment),
        // pop_clip, parent exit.
        let expected = [
            ">> ",       // parent entry
            "push_clip", // PushClip
            "<< node",   // parent exit (1st segment)
            ">> ",       // child entry
            "fill",      // FillGeometry
            "<< node",   // child exit
            ">> ",       // parent entry (2nd segment)
            "pop_clip",  // PopClip
            "<< node",   // parent exit
        ];
        assert_eq!(canvas.calls.len(), expected.len(), "got {:?}", canvas.calls);
        for (i, (got, exp)) in canvas.calls.iter().zip(expected.iter()).enumerate() {
            assert!(
                got.starts_with(exp),
                "call #{i}: got {got:?}, expected prefix {exp:?}",
            );
        }
        assert_eq!(canvas.transform_depth, 0);
    }

    /// Record-then-playback roundtrip: recording into a `RecordingCanvas`
    /// and then playing the resulting commands back through a
    /// `CapturingCanvas` should produce the same op stream as recording
    /// directly into the capturing canvas (modulo the `>>`/`<<` node
    /// markers, which the recording splits into per-Paint boundaries).
    #[test]
    fn record_then_playback_roundtrip() {
        // Record pass: simulate a parent + child paint walk.
        let mut recorder = RecordingCanvas::new();
        recorder.notify_node_entry(nid(1), Affine::IDENTITY, Size::new(100.0, 100.0));
        recorder.push_clip(Offset::ZERO, Size::new(100.0, 100.0));
        recorder.notify_node_entry(nid(2), Affine::IDENTITY, Size::new(50.0, 50.0));
        recorder.fill_geometry(
            Offset::ZERO,
            &Geometry::Rect(Size::new(50.0, 50.0)),
            &Brush::SolidColor(Color::rgb(0, 255, 0)),
        );
        recorder.notify_node_exit();
        recorder.pop_clip();
        recorder.notify_node_exit();
        let commands = recorder.into_render_commands();

        // Playback into capturing canvas.
        let mut captured = CapturingCanvas::new();
        play_commands(&mut captured, &commands);

        // The captured stream must contain push_clip → fill → pop_clip in
        // order, with notify markers bracketing each Paint. Three Paint
        // commands → 3 entry/exit pairs + 3 ops = 9 calls.
        assert_eq!(captured.calls.len(), 9, "got {:?}", captured.calls);
        assert_eq!(captured.transform_depth, 0, "unbalanced notify markers");
    }
}
