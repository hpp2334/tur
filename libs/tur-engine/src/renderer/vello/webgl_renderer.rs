//! WebGL2-backed vello-hybrid renderer (wasm32 browser target).
//!
//! This module is only compiled when the `webgl` feature is active. It wraps
//! vello-hybrid's [`WebGlRenderer`], which renders directly to the browser
//! canvas via the native `WebGl2RenderingContext` — no wgpu dependency, ~3MB
//! smaller binary than the WebGPU path.

use std::collections::HashMap;

use crate::core::element::ElementNodeId;
use crate::core::elements::NodeTreeData;
use crate::core::render::Renderer as TurRenderer;
use crate::core::resource::{ResourceId, ResourceMap};
use crate::core::shell::PaintShell;
use crate::renderer::vello::scene_paint::{new_scene, paint_tree_to_scene};
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
    /// Cache mapping each registered image resource to its uploaded hybrid
    /// `ImageId`. The WebGL backend only supports `ImageSource::OpaqueId`, so
    /// every image must be uploaded to the atlas before painting.
    image_uploads: HashMap<ResourceId, ImageId>,
}

impl WebGlVelloRenderer {
    /// Create a new WebGL renderer bound to the given canvas.
    ///
    /// The canvas's backing buffer (`width`/`height` attributes) must already
    /// be set to the physical pixel dimensions (`logical * dpr`).
    pub fn new(canvas: HtmlCanvasElement, logical_width: u32, logical_height: u32, dpr: f64) -> Self {
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
            image_uploads: HashMap::new(),
        }
    }

    fn render_to_scene(
        &mut self,
        tree: &NodeTreeData,
        focused_node_id: Option<ElementNodeId>,
        resource_map: &ResourceMap,
        shell: PaintShell<'_>,
    ) {
        self.upload_images(resource_map);

        paint_tree_to_scene(
            &mut self.scene,
            &mut self.resources,
            &self.image_uploads,
            self.physical_width,
            self.physical_height,
            self.dpr,
            tree,
            focused_node_id,
            resource_map,
            shell,
        );
    }

    /// Upload any new image resources to the hybrid image cache (atlas),
    /// caching their `ImageId` keyed by `ResourceId`. Stale entries (images no
    /// longer in the resource map) are pruned from the cache.
    fn upload_images(&mut self, resource_map: &ResourceMap) {
        for (rid, img_res) in resource_map.iter_images() {
            if self.image_uploads.contains_key(&rid) {
                continue;
            }
            let source = ImageSource::from_peniko_image_data(&img_res.peniko_image);
            let pixmap = match source {
                ImageSource::Pixmap(p) => p,
                _ => continue,
            };
            let image_id = self.renderer.upload_image(&mut self.resources, &pixmap);
            self.image_uploads.insert(rid, image_id);
        }
        self.image_uploads.retain(|rid, _| resource_map.has_image(*rid));
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
    fn render(
        &mut self,
        tree: &NodeTreeData,
        focused_node_id: Option<ElementNodeId>,
        resource_map: &ResourceMap,
        shell: PaintShell<'_>,
    ) {
        self.render_to_scene(tree, focused_node_id, resource_map, shell);
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
}
