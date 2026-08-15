//! No-op renderer — factory + target that discard paint batches (headless
//! instances), logging per-op stats for debugging.

use std::collections::HashMap;

use crate::core::render::{
    NullCanvas, RenderCommand, RenderTarget, Renderer, Surface, SurfaceHandle, play_commands,
};

/// Opaque surface payload accepted by [`NoopRenderer`] — the unit type.
/// Pass `Box::new(NoopSurface)` to `setup_root(...)` for a sized headless
/// root (the root's `Screen` + `viewportSize$` still work; batches are
/// discarded after stats logging).
pub struct NoopSurface;

impl Surface for NoopSurface {
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

pub struct NoopRenderer;

impl Default for NoopRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl NoopRenderer {
    pub fn new() -> Self {
        NoopRenderer
    }
}

impl Renderer for NoopRenderer {
    fn create_target(
        &mut self,
        surface: SurfaceHandle,
        _viewport: (f64, f64),
        _dpr: f64,
    ) -> Result<Box<dyn RenderTarget>, crate::error::TurError> {
        let _ = crate::core::render::downcast_surface::<NoopSurface>("NoopRenderer", surface)?;
        Ok(Box::new(NoopTarget))
    }
}

pub struct NoopTarget;

impl RenderTarget for NoopTarget {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        // Drive playback against a null canvas so any side effects baked
        // into Canvas ops (none today, but defensive) still run. Paint
        // counts come from the recorded batch.
        let mut null = NullCanvas;
        play_commands(&mut null, commands);

        let mut counts: HashMap<&str, usize> = HashMap::new();
        for cmd in commands {
            let RenderCommand::Paint { ops, .. } = cmd;
            for op in ops {
                let key: &str = match op {
                    crate::core::render::CanvasOp::FillGeometry { .. } => "fill",
                    crate::core::render::CanvasOp::StrokeGeometry { .. } => "stroke",
                    crate::core::render::CanvasOp::FillTextLayout { .. } => "text",
                    crate::core::render::CanvasOp::DrawImage { .. } => "image",
                    crate::core::render::CanvasOp::DrawShadow { .. } => "shadow",
                    crate::core::render::CanvasOp::PushClip { .. }
                    | crate::core::render::CanvasOp::PushClipGeometry { .. } => "clip",
                    crate::core::render::CanvasOp::PopClip => "pop_clip",
                    crate::core::render::CanvasOp::PushOpacity(_) => "opacity",
                    crate::core::render::CanvasOp::PopOpacity => "pop_opacity",
                    crate::core::render::CanvasOp::PushTransform(_) => "transform",
                    crate::core::render::CanvasOp::PopTransform => "pop_transform",
                };
                *counts.entry(key).or_insert(0) += 1;
            }
        }
        let paint_count = commands.len();
        let total_ops: usize = counts.values().sum();
        tracing::debug!(
            "noop-renderer: {paint_count} Paint commands, {total_ops} total ops, breakdown: {:?}",
            counts
        );
    }
}
