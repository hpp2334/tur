//! WebGL2-backed vello-hybrid renderer (wasm32 browser target).
//!
//! This module is only compiled when the `webgl` feature is active. It wraps
//! vello-hybrid's [`WebGlRenderer`], which renders directly to the browser
//! canvas via the native `WebGl2RenderingContext` — no wgpu dependency, ~3MB
//! smaller binary than the WebGPU path.
//!
//! The engine runs on a worker thread and ships `Vec<RenderCommand>` to
//! main; main applies the batch via [`WebGlVelloRenderer::render_commands`].

use std::collections::HashMap;

use crate::core::image_resource::{ImageResource, ImageResourceId};
use crate::core::render::RenderCommand;
use crate::core::render::Renderer as TurRenderer;
use crate::core::render::brush::Color;
use crate::renderer::vello::scene_paint::{new_scene, paint_commands_to_scene};
use vello_common::paint::{ImageId, ImageSource};
use vello_hybrid::{RenderSize, Resources, Scene, WebGlRenderer};
use web_sys::HtmlCanvasElement;

pub struct WebGlVelloRenderer {
    renderer: WebGlRenderer,
    scene: Scene,
    resources: Resources,
    dpr: f64,
    physical_width: u32,
    physical_height: u32,
    /// Base (background) color painted as the scene's first element each
    /// frame. vello-hybrid clears the target to transparent, so this is the
    /// frame's effective clear color. Defaults to opaque white
    /// ([`Color::WHITE`]).
    base_color: Color,
    /// Cache mapping each registered image resource to its uploaded hybrid
    /// `ImageId`. The WebGL backend only supports `ImageSource::OpaqueId`, so
    /// every image must be uploaded to the atlas before painting.
    image_uploads: HashMap<ImageResourceId, ImageId>,
}

impl WebGlVelloRenderer {
    /// Create a new WebGL renderer bound to the given canvas.
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

        let renderer = WebGlRenderer::new(&canvas);
        let scene = new_scene(physical_width, physical_height);

        WebGlVelloRenderer {
            renderer,
            scene,
            resources: Resources::new(),
            dpr,
            physical_width,
            physical_height,
            base_color: Color::WHITE,
            image_uploads: HashMap::new(),
        }
    }

    /// Set the base (background) color, consuming and returning `self` for
    /// use before the renderer is boxed into `dyn Renderer`.
    ///
    /// Applied at paint time — the next rendered frame carries the new color
    /// (no repaint is triggered by this call).
    pub fn with_base_color(mut self, color: Color) -> Self {
        self.base_color = color;
        self
    }

    /// Set the base (background) color on a renderer still held concretely.
    pub fn set_base_color(&mut self, color: Color) {
        self.base_color = color;
    }

    /// The configured base (background) color.
    pub fn base_color(&self) -> Color {
        self.base_color
    }

    /// Render a flat command batch into the scene. Playback happens in
    /// `paint_commands_to_scene`; image upload happens incrementally via
    /// `TurRenderer::upload_image_resource` as the worker registers
    /// resources.
    fn render_commands_to_scene(&mut self, commands: &[RenderCommand]) {
        paint_commands_to_scene(
            &mut self.scene,
            &mut self.resources,
            &self.image_uploads,
            self.physical_width,
            self.physical_height,
            self.dpr,
            &self.base_color,
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

impl TurRenderer for WebGlVelloRenderer {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        // Surface geometry is tracked on `self` (synced via `resize`, which
        // fires on viewport-change events only).
        self.render_commands_to_scene(commands);
    }

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        WebGlVelloRenderer::present(self);
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
