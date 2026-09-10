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
    /// primary path). The renderer resets the scene, fills the configured
    /// base color (opaque white by default — see the concrete renderers'
    /// `with_base_color`), seeds a `VelloPaintContext` with
    /// `Affine::scale(dpr)` as the root transform, and plays the commands
    /// back via [`crate::core::render::play_commands`].
    ///
    /// Surface geometry lives on `self` — the engine syncs it via
    /// [`Self::resize`] immediately before this method, at the render
    /// commit point, so geometry and content land in one operation — no
    /// dimensions are passed. Images are uploaded incrementally via
    /// [`Self::upload_image_resource`] as the worker registers them; the
    /// command batch itself only carries `ImageResourceId`s.
    ///
    /// Cursor claims happen during the worker-side recording pass; main
    /// replays commands without re-claiming, so no `PaintEnv` is needed
    /// here.
    fn render_commands(&mut self, commands: &[RenderCommand]);

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Reconfigure the backing store to the given geometry (logical size +
    /// device pixel ratio).
    ///
    /// The engine calls this ONLY at the render commit point —
    /// immediately before [`Self::render_commands`], with the viewport the
    /// batch about to be played was laid out for (deduped, so a steady
    /// frame stream never reconfigures). That ordering is the whole
    /// contract: resizing a backing store (the WebGL canvas — setting
    /// `width`/`height` resets the bitmap synchronously; a wgpu swapchain)
    /// destroys the presented frame, so doing it anywhere earlier leaves a
    /// cleared surface composited until the replacement frame lands (the
    /// resize white flash). The old frame stays visible (CSS-stretched on
    /// the web) until the new frame swaps it out in the same operation.
    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}

    /// Upload (or refresh) one image resource in the GPU atlas. Called once
    /// per newly-registered resource (`HostMsg::UploadImage`), replacing
    /// the old per-frame full-map upload sweep. Default: no-op.
    fn upload_image_resource(&mut self, _id: ImageResourceId, _image: &ImageResource) {}

    fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        None
    }
}
