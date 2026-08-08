use crate::core::image_resource::{ImageResource, ImageResourceId};
use crate::core::render::RenderCommand;

/// Engine → backend rendering contract.
///
/// Production path: the worker records paint ops via
/// [`crate::core::render::RecordingCanvas`], post-processes the recording
/// into `Vec<RenderCommand>` (one or more `Paint`s per node, in playback
/// order), and main plays the batch back linearly via
/// [`crate::core::render::play_commands`] using [`Self::render_commands`].
///
/// Concrete renderers implement [`Self::render_commands`]. They typically
/// share scene/canvas setup and play the command batch through a shared
/// helper.
pub trait Renderer {
    /// Command-batch path: render from a flat command batch (the new
    /// primary path). The renderer resets the scene, fills the default
    /// white background, seeds a `VelloPaintContext` with
    /// `Affine::scale(dpr)` as the root transform, and plays the commands
    /// back via [`crate::core::render::play_commands`].
    ///
    /// Surface geometry lives on `self` (kept in sync via [`Self::resize`],
    /// which fires only on viewport-change events) — no dimensions are
    /// passed. Images are uploaded incrementally via
    /// [`Self::upload_image_resource`] as the worker registers them; the
    /// command batch itself only carries `ImageResourceId`s.
    ///
    /// Cursor claims happen during the worker-side recording pass; main
    /// replays commands without re-claiming, so no `PaintShell` is needed
    /// here.
    fn render_commands(&mut self, commands: &[RenderCommand]);

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}

    /// Upload (or refresh) one image resource in the GPU atlas. Called once
    /// per newly-registered resource (`MainMsg::UploadImage`), replacing
    /// the old per-frame full-map upload sweep. Default: no-op.
    fn upload_image_resource(&mut self, _id: ImageResourceId, _image: &ImageResource) {}

    fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        None
    }
}
