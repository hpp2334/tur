//! wgpu-backed vello-hybrid renderer (native + WebGPU targets).
//!
//! This module is only compiled when the `wgpu-backend` feature is active.

use crate::core::element::ElementNodeId;
use crate::core::elements::NodeTreeData;
use crate::core::image_resource::{ImageResourceId, ImageResourceMap};
use crate::core::render::RenderCommand;
use crate::core::render::Renderer as TurRenderer;
use crate::core::shell::PaintShell;
use crate::renderer::vello::scene_paint::{
    new_scene, paint_commands_to_scene, paint_tree_to_scene,
};
use std::collections::HashMap;
use vello_common::paint::{ImageId, ImageSource};
use vello_hybrid::{RenderSize, RenderTargetConfig, Renderer, Resources, Scene, TextureBindings};

#[derive(Debug, thiserror::Error)]
pub enum VelloRendererError {
    #[error("vello render failed: {0}")]
    Render(#[source] vello_hybrid::RenderError),
}

pub struct VelloRenderer {
    renderer: Renderer,
    scene: Scene,
    resources: Resources,
    texture_bindings: TextureBindings,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    dpr: f64,
    physical_width: u32,
    physical_height: u32,
    max_texture_dimension: u32,
    /// Cache mapping each registered image resource to its uploaded hybrid
    /// `ImageId`. The WebGPU backend only supports `ImageSource::OpaqueId`, so
    /// every image must be uploaded to the atlas before painting.
    image_uploads: HashMap<ImageResourceId, ImageId>,
}

impl VelloRenderer {
    pub fn init_surface(
        adapter: &wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        logical_width: u32,
        logical_height: u32,
        dpr: f64,
    ) -> Self {
        let max_texture_dimension = device.limits().max_texture_dimension_2d;
        let physical_width = ((logical_width as f64 * dpr) as u32).min(max_texture_dimension);
        let physical_height = ((logical_height as f64 * dpr) as u32).min(max_texture_dimension);

        let mut config = surface
            .get_default_config(adapter, physical_width, physical_height)
            .expect("failed to get default surface config");

        let surface_format = {
            let caps = surface.get_capabilities(adapter);
            caps.formats
                .iter()
                .find(|f| {
                    matches!(
                        f,
                        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
                    )
                })
                .or_else(|| caps.formats.iter().find(|f| f.is_srgb()))
                .copied()
                .unwrap_or_else(|| {
                    caps.formats
                        .first()
                        .copied()
                        .expect("no surface formats available")
                })
        };
        config.format = surface_format;
        config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT;

        surface.configure(&device, &config);

        let render_target_config = RenderTargetConfig {
            format: surface_format,
            width: physical_width,
            height: physical_height,
        };
        let renderer = Renderer::new(&device, &render_target_config);

        let scene = new_scene(physical_width, physical_height);

        VelloRenderer {
            renderer,
            scene,
            resources: Resources::new(),
            texture_bindings: TextureBindings::new(),
            device,
            queue,
            surface,
            config,
            dpr,
            physical_width,
            physical_height,
            max_texture_dimension,
            image_uploads: HashMap::new(),
        }
    }

    pub fn resize(&mut self, logical_width: u32, logical_height: u32, dpr: f64) {
        self.dpr = dpr;
        self.physical_width = ((logical_width as f64 * dpr) as u32).min(self.max_texture_dimension);
        self.physical_height =
            ((logical_height as f64 * dpr) as u32).min(self.max_texture_dimension);

        self.config.width = self.physical_width;
        self.config.height = self.physical_height;
        self.surface.configure(&self.device, &self.config);

        // The hybrid `Scene` is created with fixed pixel dimensions, so it must be
        // recreated on resize.
        self.scene = new_scene(self.physical_width, self.physical_height);
    }

    pub fn render_to_scene(
        &mut self,
        tree: &NodeTreeData,
        focused_node_id: Option<ElementNodeId>,
        image_resource_map: &ImageResourceMap,
        shell: PaintShell<'_>,
    ) {
        // The WebGPU backend only supports `ImageSource::OpaqueId`, so upload
        // any image resources that are not yet cached before painting.
        self.upload_images(image_resource_map);

        paint_tree_to_scene(
            &mut self.scene,
            &mut self.resources,
            &self.image_uploads,
            self.physical_width,
            self.physical_height,
            self.dpr,
            tree,
            focused_node_id,
            image_resource_map,
            shell,
        );
    }

    /// New record/playback path: render a flat command batch into the scene.
    /// Used by `TurRenderer::render_commands`. Image upload is performed
    /// here (same as `render_to_scene`); playback itself happens in
    /// `paint_commands_to_scene`.
    pub fn render_commands_to_scene(
        &mut self,
        commands: &[RenderCommand],
        image_resource_map: &ImageResourceMap,
        shell: PaintShell<'_>,
    ) {
        self.upload_images(image_resource_map);
        paint_commands_to_scene(
            &mut self.scene,
            &mut self.resources,
            &self.image_uploads,
            self.physical_width,
            self.physical_height,
            self.dpr,
            commands,
            shell,
        );
    }

    /// Upload any new image resources to the hybrid image cache (atlas),
    /// caching their `ImageId` keyed by `ImageResourceId`. Stale entries (images no
    /// longer in the resource map) are pruned from the cache.
    fn upload_images(&mut self, image_resource_map: &ImageResourceMap) {
        let VelloRenderer {
            renderer,
            resources,
            device,
            queue,
            image_uploads,
            ..
        } = self;

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("image upload"),
        });
        let mut uploaded_any = false;
        for (rid, img_res) in image_resource_map.iter_images() {
            if image_uploads.contains_key(&rid) {
                continue;
            }
            let source = ImageSource::from_peniko_image_data(&img_res.peniko_image);
            let pixmap = match source {
                ImageSource::Pixmap(p) => p,
                // Only inline pixmap sources are produced from decoded image data.
                _ => continue,
            };
            let image_id = renderer.upload_image(resources, device, queue, &mut encoder, &pixmap);
            image_uploads.insert(rid, image_id);
            uploaded_any = true;
        }
        // Prune stale entries so removed images don't keep atlas slots forever.
        image_uploads.retain(|rid, _| image_resource_map.has_image(*rid));

        if uploaded_any {
            queue.submit(std::iter::once(encoder.finish()));
        }
    }

    pub fn present(&mut self) -> Result<(), VelloRendererError> {
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Timeout => {
                tracing::warn!("present: get_current_texture timed out");
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                tracing::warn!("present: get_current_texture outdated, reconfiguring surface");
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            other => {
                tracing::warn!("present: get_current_texture returned {other:?}");
                return Ok(());
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let render_size = RenderSize {
            width: self.physical_width,
            height: self.physical_height,
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("vello hybrid render"),
            });

        if let Err(e) = self.renderer.render(
            &self.scene,
            &mut self.resources,
            &self.device,
            &self.queue,
            &mut encoder,
            &render_size,
            &view,
            &self.texture_bindings,
        ) {
            tracing::error!("present: vello hybrid render failed: {e}");
            return Err(VelloRendererError::Render(e));
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn render_to_pixels(&mut self) -> Vec<u8> {
        // The hybrid renderer is bound to one target format (the surface
        // format), so the offscreen texture must use that same format. The
        // result is converted to RGBA8 byte order for consumers.
        let format = self.config.format;
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vello hybrid readback"),
            size: wgpu::Extent3d {
                width: self.physical_width,
                height: self.physical_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let render_size = RenderSize {
            width: self.physical_width,
            height: self.physical_height,
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("vello hybrid readback"),
            });

        if let Err(e) = self.renderer.render(
            &self.scene,
            &mut self.resources,
            &self.device,
            &self.queue,
            &mut encoder,
            &render_size,
            &view,
            &self.texture_bindings,
        ) {
            tracing::error!("render_to_pixels: vello hybrid render failed: {e}");
            return Vec::new();
        }

        let bytes_per_row_aligned = ((self.physical_width * 4).div_ceil(256)) * 256;
        let buffer_size = (bytes_per_row_aligned as u64) * (self.physical_height as u64);
        let readback_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row_aligned),
                    rows_per_image: Some(self.physical_height),
                },
            },
            wgpu::Extent3d {
                width: self.physical_width,
                height: self.physical_height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();

        let data = slice.get_mapped_range();
        let swap_red_blue = matches!(
            format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let mut pixels =
            Vec::with_capacity((self.physical_width * self.physical_height * 4) as usize);
        for row in 0..self.physical_height {
            let offset = row as usize * bytes_per_row_aligned as usize;
            let row_end = offset + (self.physical_width * 4) as usize;
            if swap_red_blue {
                for chunk in data[offset..row_end].chunks_exact(4) {
                    pixels.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
                }
            } else {
                pixels.extend_from_slice(&data[offset..row_end]);
            }
        }
        pixels
    }
}

impl TurRenderer for VelloRenderer {
    fn render(
        &mut self,
        tree: &NodeTreeData,
        focused_node_id: Option<ElementNodeId>,
        image_resource_map: &ImageResourceMap,
        shell: PaintShell<'_>,
    ) {
        self.render_to_scene(tree, focused_node_id, image_resource_map, shell);
    }

    fn render_commands(
        &mut self,
        commands: &[RenderCommand],
        _physical_width: u32,
        _physical_height: u32,
        _dpr: f64,
        image_resource_map: &ImageResourceMap,
        shell: PaintShell<'_>,
    ) {
        // `physical_width` / `physical_height` / `dpr` are already tracked
        // on `self` (kept in sync via `resize`), so the trait-level args
        // are ignored here. They're threaded through the trait for
        // renderers that don't own their surface geometry (future main-side
        // renderer in Phase 7).
        self.render_commands_to_scene(commands, image_resource_map, shell);
    }

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        VelloRenderer::present(self).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn resize(&mut self, logical_width: u32, logical_height: u32, dpr: f64) {
        VelloRenderer::resize(self, logical_width, logical_height, dpr);
    }

    fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        Some(VelloRenderer::render_to_pixels(self))
    }
}
