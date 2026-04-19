use std::num::NonZeroUsize;

use crate::core::elements::ElementTree;
use crate::core::render::Renderer as TurRenderer;
use crate::renderer::vello::paint_context::VelloPaintContext;
use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Mix};
use vello::wgpu::{SurfaceConfiguration, TextureUsages};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};

#[derive(Debug, thiserror::Error)]
pub enum VelloRendererError {
    #[error("vello render failed: {0}")]
    Render(#[source] vello::Error),
}

pub struct VelloRenderer {
    renderer: Renderer,
    scene: Scene,
    device: vello::wgpu::Device,
    queue: vello::wgpu::Queue,
    surface: vello::wgpu::Surface<'static>,
    config: SurfaceConfiguration,
    dpr: f64,
    physical_width: u32,
    physical_height: u32,
}

impl VelloRenderer {
    pub fn init_surface(
        adapter: &vello::wgpu::Adapter,
        device: vello::wgpu::Device,
        queue: vello::wgpu::Queue,
        surface: vello::wgpu::Surface<'static>,
        logical_width: u32,
        logical_height: u32,
        dpr: f64,
    ) -> Self {
        let physical_width = (logical_width as f64 * dpr) as u32;
        let physical_height = (logical_height as f64 * dpr) as u32;

        let mut config = surface
            .get_default_config(adapter, physical_width, physical_height)
            .expect("failed to get default surface config");

        let surface_format = {
            let caps = surface.get_capabilities(adapter);
            caps.formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(config.format)
        };
        config.format = surface_format;
        config.usage = TextureUsages::RENDER_ATTACHMENT;

        surface.configure(&device, &config);

        let options = RendererOptions {
            surface_format: Some(surface_format),
            use_cpu: false,
            antialiasing_support: AaSupport::all(),
            num_init_threads: NonZeroUsize::new(1),
        };
        let renderer = Renderer::new(&device, options).expect("failed to create vello Renderer");

        VelloRenderer {
            renderer,
            scene: Scene::new(),
            device,
            queue,
            surface,
            config,
            dpr,
            physical_width,
            physical_height,
        }
    }

    pub fn resize(&mut self, logical_width: u32, logical_height: u32, dpr: f64) {
        self.dpr = dpr;
        self.physical_width = (logical_width as f64 * dpr) as u32;
        self.physical_height = (logical_height as f64 * dpr) as u32;

        self.config.width = self.physical_width;
        self.config.height = self.physical_height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render_to_scene(&mut self, tree: &ElementTree) {
        self.scene.reset();
        if self.dpr != 1.0 {
            self.scene.push_layer(
                Mix::Normal,
                1.0,
                Affine::scale(self.dpr),
                &Rect::new(0.0, 0.0, f64::MAX, f64::MAX),
            );
        }
        let mut ctx = VelloPaintContext::new(&mut self.scene);
        tree.paint(&mut ctx);
        if self.dpr != 1.0 {
            self.scene.pop_layer();
        }
    }

    pub fn present(&mut self) -> Result<(), VelloRendererError> {
        let output = self
            .surface
            .get_current_texture()
            .expect("failed to get surface texture");

        let params = RenderParams {
            base_color: Color::from_rgba8(0, 0, 0, 255),
            width: self.physical_width,
            height: self.physical_height,
            antialiasing_method: AaConfig::Msaa8,
        };

        self.renderer
            .render_to_surface(&self.device, &self.queue, &self.scene, &output, &params)
            .map_err(VelloRendererError::Render)?;

        output.present();
        Ok(())
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }
}

impl TurRenderer for VelloRenderer {
    fn render(&mut self, tree: &ElementTree) {
        self.render_to_scene(tree);
    }

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        VelloRenderer::present(self).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn resize(&mut self, logical_width: u32, logical_height: u32, dpr: f64) {
        VelloRenderer::resize(self, logical_width, logical_height, dpr);
    }
}
