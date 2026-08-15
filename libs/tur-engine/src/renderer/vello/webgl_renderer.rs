//! WebGL2-backed vello-hybrid rendering (wasm32 browser target).
//!
//! Split into a **factory** ([`WebGlRenderer`] — unit) and a **target**
//! ([`WebGlTarget`] — one canvas per view root, wrapping vello-hybrid's
//! `WebGlRenderer` which renders directly to the browser canvas via the
//! native `WebGl2RenderingContext` — no wgpu dependency, ~3MB smaller binary
//! than the WebGPU path).
//!
//! The engine runs on a worker thread and ships `Vec<RenderCommand>` to
//! main; main applies the batch to the owning root's
//! [`WebGlTarget::render_commands`].

use std::collections::HashMap;

use crate::core::image_resource::{ImageResource, ImageResourceId};
use crate::core::render::RenderCommand;
use crate::core::render::RenderTarget as TurRenderTarget;
use crate::core::render::Renderer as TurRenderer;
use crate::core::render::{Surface, SurfaceHandle, downcast_surface};
use crate::renderer::vello::scene_paint::{new_scene, paint_commands_to_scene};
use vello_common::paint::{ImageId, ImageSource};
use vello_hybrid::{RenderSize, Resources, Scene, WebGlRenderer as VelloWebGlRenderer};
use web_sys::HtmlCanvasElement;

/// Opaque surface payload accepted by [`WebGlRenderer`] — the browser canvas
/// to render into.
pub struct CanvasSurface(pub HtmlCanvasElement);

impl Surface for CanvasSurface {
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

/// Factory for [`WebGlTarget`]s. Unit type — each WebGL canvas carries its
/// own GL context, so there is no shared substrate.
pub struct WebGlRenderer;

impl Default for WebGlRenderer {
    fn default() -> Self {
        Self
    }
}

impl WebGlRenderer {
    pub fn new() -> Self {
        WebGlRenderer
    }
}

impl TurRenderer for WebGlRenderer {
    fn create_target(
        &mut self,
        surface: SurfaceHandle,
        viewport: (f64, f64),
        dpr: f64,
    ) -> Result<Box<dyn TurRenderTarget>, crate::error::TurError> {
        let canvas = downcast_surface::<CanvasSurface>("WebGlRenderer", surface)?;
        Ok(Box::new(WebGlTarget::new(
            canvas.0,
            viewport.0 as u32,
            viewport.1 as u32,
            dpr,
        )))
    }
}

/// One WebGL2 render target bound to a single canvas.
pub struct WebGlTarget {
    renderer: VelloWebGlRenderer,
    scene: Scene,
    resources: Resources,
    dpr: f64,
    physical_width: u32,
    physical_height: u32,
    /// Cache mapping each registered image resource to its uploaded hybrid
    /// `ImageId`. The WebGL backend only supports `ImageSource::OpaqueId`, so
    /// every image must be uploaded to the atlas before painting.
    image_uploads: HashMap<ImageResourceId, ImageId>,
}

impl WebGlTarget {
    /// Create a new target bound to the given canvas.
    ///
    /// The canvas's backing buffer (`width`/`height` attributes) must already
    /// be set to the physical pixel dimensions (`logical * dpr`).
    pub fn new(
        canvas: HtmlCanvasElement,
        logical_width: u32,
        logical_height: u32,
        dpr: f64,
    ) -> Self {
        let physical_width = (logical_width as f64 * dpr) as u32;
        let physical_height = (logical_height as f64 * dpr) as u32;

        let renderer = VelloWebGlRenderer::new(&canvas);
        let scene = new_scene(physical_width, physical_height);

        WebGlTarget {
            renderer,
            scene,
            resources: Resources::new(),
            dpr,
            physical_width,
            physical_height,
            image_uploads: HashMap::new(),
        }
    }

    /// Render a flat command batch into the scene. Playback happens in
    /// `paint_commands_to_scene`; image upload happens incrementally via
    /// `TurRenderTarget::upload_image_resource` as the worker registers
    /// resources.
    fn render_commands_to_scene(&mut self, commands: &[RenderCommand]) {
        paint_commands_to_scene(
            &mut self.scene,
            &mut self.resources,
            &self.image_uploads,
            self.physical_width,
            self.physical_height,
            self.dpr,
            commands,
        );
    }

    fn present(&mut self) {
        let render_size = RenderSize {
            width: self.physical_width,
            height: self.physical_height,
        };
        if let Err(e) = self
            .renderer
            .render(&self.scene, &mut self.resources, &render_size)
        {
            tracing::error!("present: vello hybrid webgl render failed: {e}");
        }
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }
}

impl TurRenderTarget for WebGlTarget {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        // Surface geometry is tracked on `self` (synced via `resize`, which
        // fires on viewport-change events only).
        self.render_commands_to_scene(commands);
    }

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        WebGlTarget::present(self);
        Ok(())
    }

    fn resize(&mut self, logical_width: u32, logical_height: u32, dpr: f64) {
        self.dpr = dpr;
        self.physical_width = (logical_width as f64 * dpr) as u32;
        self.physical_height = (logical_height as f64 * dpr) as u32;
        // The hybrid `Scene` is created with fixed pixel dimensions, so it must
        // be recreated on resize.
        self.scene = new_scene(self.physical_width, self.physical_height);
    }

    fn upload_image_resource(&mut self, id: ImageResourceId, image: &ImageResource) {
        if self.image_uploads.contains_key(&id) {
            return;
        }
        let source = ImageSource::from_peniko_image_data(&image.peniko_image);
        let pixmap = match source {
            ImageSource::Pixmap(p) => p,
            _ => return,
        };
        let image_id = self.renderer.upload_image(&mut self.resources, &pixmap);
        self.image_uploads.insert(id, image_id);
    }
}
