use crate::paint_context::{paint_tree, PaintContext};
use tur_render_tree::{RenderTree, Renderer as TurRenderer};
use vello::{AaSupport, RenderParams, Renderer, RendererOptions, Scene};

#[derive(Debug, thiserror::Error)]
pub enum VelloRendererError {
    #[error("renderer not initialized, call ensure_renderer first")]
    NotInitialized,
    #[error("vello render failed: {0}")]
    Render(#[source] vello::Error),
}

pub struct VelloRenderer {
    renderer: Option<Renderer>,
    scene: Scene,
}

impl Default for VelloRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl VelloRenderer {
    pub fn new() -> Self {
        VelloRenderer {
            renderer: None,
            scene: Scene::new(),
        }
    }

    pub fn render_to_scene(&mut self, tree: &RenderTree) {
        self.scene.reset();
        let mut ctx = PaintContext::new(&mut self.scene);
        paint_tree(&mut ctx, tree);
    }
}

impl TurRenderer for VelloRenderer {
    fn render(&mut self, tree: &RenderTree) {
        self.render_to_scene(tree);
    }
}

impl VelloRenderer {
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn ensure_renderer(
        &mut self,
        device: &vello::wgpu::Device,
        surface_format: vello::wgpu::TextureFormat,
    ) {
        if self.renderer.is_none() {
            let options = RendererOptions {
                surface_format: Some(surface_format),
                use_cpu: false,
                antialiasing_support: AaSupport::all(),
                num_init_threads: NonZeroUsize::new(1),
            };
            self.renderer =
                Some(Renderer::new(device, options).expect("failed to create vello Renderer"));
        }
    }

    pub fn render_to_surface(
        &mut self,
        device: &vello::wgpu::Device,
        queue: &vello::wgpu::Queue,
        surface_texture: &vello::wgpu::SurfaceTexture,
        width: u32,
        height: u32,
    ) -> Result<(), VelloRendererError> {
        let renderer = self
            .renderer
            .as_mut()
            .ok_or(VelloRendererError::NotInitialized)?;

        let params = RenderParams {
            base_color: vello::peniko::Color::from_rgba8(0, 0, 0, 255),
            width,
            height,
            antialiasing_method: vello::AaConfig::Msaa8,
        };

        renderer
            .render_to_surface(device, queue, &self.scene, surface_texture, &params)
            .map_err(VelloRendererError::Render)?;
        Ok(())
    }
}

use std::num::NonZeroUsize;
