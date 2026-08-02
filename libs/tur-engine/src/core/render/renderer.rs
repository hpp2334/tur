use crate::core::element::ElementNodeId;
use crate::core::elements::NodeTreeData;
use crate::core::image_resource::ImageResourceMap;
use crate::core::render::RenderCommand;
use crate::core::shell::PaintShell;

/// Engine → backend rendering contract.
///
/// Two entry points coexist in Phase 3:
///
/// - **[`Self::render`] (direct path)** — render directly from a borrowed
///   [`NodeTreeData`]. The legacy path used before Phase 3, kept available
///   for parity testing via the `direct-render` feature on `tur-engine`.
///   Production code prefers [`Self::render_commands`].
///
/// - **[`Self::render_commands`] (command-batch path)** — render from a
///   flat `&[RenderCommand]` produced by the record pass
///   ([`crate::core::render::RecordingCanvas`]). This is the new primary
///   path: the worker records into a `RecordingCanvas`, post-processes
///   into `Vec<RenderCommand>` (one or more `Paint`s per node, in playback
///   order), and main plays the batch back linearly via
///   [`crate::core::render::play_commands`]. Phase 3 runs both sides on
///   the same thread; Phase 7 splits worker/main.
///
/// Concrete renderers implement both. Implementations typically share
/// the scene/canvas setup and differ only in the source (tree vs commands).
pub trait Renderer {
    /// Direct path: render from the live element tree. Used when the
    /// `direct-render` cargo feature is enabled (parity testing). The
    /// renderer walks the tree itself (or delegates to a shared helper
    /// like `paint_tree_to_scene`).
    fn render(
        &mut self,
        tree: &NodeTreeData,
        focused_node_id: Option<ElementNodeId>,
        image_resource_map: &ImageResourceMap,
        shell: PaintShell<'_>,
    );

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
    /// (used to upload new images); `shell` exposes cursor + clock to the
    /// paint pass.
    fn render_commands(
        &mut self,
        commands: &[RenderCommand],
        physical_width: u32,
        physical_height: u32,
        dpr: f64,
        image_resource_map: &ImageResourceMap,
        shell: PaintShell<'_>,
    );

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}

    fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        None
    }
}
