use crate::core::image_resource::ImageResourceMap;
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
    /// primary path). The renderer uploads any new image resources,
    /// resets the scene, fills the default white background, seeds a
    /// `VelloPaintContext` with `Affine::scale(dpr)` as the root
    /// transform, and plays the commands back via
    /// [`crate::core::render::play_commands`].
    ///
    /// `physical_width` / `physical_height` are the surface pixel
    /// dimensions; `dpr` is the device pixel ratio (the scale baked into
    /// the root transform). `image_resource_map` is the engine-side map
    /// (used to upload new images).
    ///
    /// Cursor claims happen during the worker-side recording pass; main
    /// replays commands without re-claiming, so no `PaintShell` is needed
    /// here.
    fn render_commands(
        &mut self,
        commands: &[RenderCommand],
        physical_width: u32,
        physical_height: u32,
        dpr: f64,
        image_resource_map: &ImageResourceMap,
    );

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}

    fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        None
    }
}
