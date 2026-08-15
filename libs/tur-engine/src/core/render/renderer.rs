//! Engine → backend rendering contract, split into a **factory** and
//! **target** layer.
//!
//! - A [`Renderer`] is created once per instance by the embedder and holds the
//!   shared backend substrate (e.g. one wgpu instance, shared shader caches).
//!   It never draws itself — it mints one [`RenderTarget`] per view root from
//!   an embedder-supplied [`SurfaceHandle`](crate::core::render::SurfaceHandle).
//! - A [`RenderTarget`] owns exactly one render target (canvas / window
//!   surface). It plays back `Vec<RenderCommand>` batches produced by the
//!   worker: the worker records paint ops via
//!   [`crate::core::render::RecordingCanvas`], post-processes the recording
//!   into commands (one or more `Paint`s per node, in playback order), and
//!   main plays the batch back linearly via
//!   [`crate::core::render::play_commands`] using [`Self::render_commands`].

use crate::core::image_resource::{ImageResource, ImageResourceId};
use crate::core::render::{RenderCommand, SurfaceHandle};

/// Factory for per-view-root [`RenderTarget`]s. One per engine instance,
/// owned by the embedder's builder call
/// (`.renderer(Box::new(...))`); lives on the main thread.
pub trait Renderer {
    /// Create a render target for one view root from the embedder-supplied
    /// opaque surface. The factory downcasts the surface to its expected
    /// concrete type and returns a clear error on a mismatched pairing.
    ///
    /// `viewport` is the initial logical `(width, height)`; `dpr` the device
    /// pixel ratio.
    fn create_target(
        &mut self,
        surface: SurfaceHandle,
        viewport: (f64, f64),
        dpr: f64,
    ) -> Result<Box<dyn RenderTarget>, crate::error::TurError>;
}

/// One render target (canvas / window surface). Created per view root via
/// [`Renderer::create_target`]; owned by
/// [`MainBackend`](crate::core::runtime::MainBackend) on the main thread.
pub trait RenderTarget {
    /// Command-batch path: render from a flat command batch. The target
    /// resets the scene, fills the default white background, seeds a
    /// `VelloPaintContext` with `Affine::scale(dpr)` as the root transform,
    /// and plays the commands back via
    /// [`crate::core::render::play_commands`].
    ///
    /// Surface geometry lives on `self` (kept in sync via [`Self::resize`],
    /// which fires only on viewport-change events) — no dimensions are
    /// passed. Images are uploaded incrementally via
    /// [`Self::upload_image_resource`] as the worker registers them; the
    /// command batch itself only carries `ImageResourceId`s.
    fn render_commands(&mut self, commands: &[RenderCommand]);

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}

    /// Upload (or refresh) one image resource in the GPU atlas. Called once
    /// per newly-registered resource (`MainMsg::UploadImage`), replacing the
    /// old per-frame full-map upload sweep. Default: no-op.
    fn upload_image_resource(&mut self, _id: ImageResourceId, _image: &ImageResource) {}

    fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        None
    }
}

/// Downcast a [`SurfaceHandle`] to `S`, producing a clear error naming both
/// types on mismatch. Shared helper for `Renderer::create_target` impls.
pub fn downcast_surface<S: crate::core::render::Surface>(
    renderer_name: &str,
    surface: SurfaceHandle,
) -> Result<S, crate::error::TurError> {
    let any: Box<dyn std::any::Any> = surface.into_any();
    match any.downcast::<S>() {
        Ok(s) => Ok(*s),
        Err(_) => Err(crate::error::TurError::Other(format!(
            "{renderer_name} expects a surface of type `{}` (got an incompatible \
             surface — pass the matching surface type to `.setup_root(...)`)",
            std::any::type_name::<S>(),
        ))),
    }
}
