//! wgpu-backed vello-hybrid rendering (native + WebGPU targets).
//!
//! Split into a **factory** ([`WgpuRenderer`] — holds the shared `wgpu`
//! instance) and a **target** ([`VelloTarget`] — one window/canvas surface
//! per view root).
//!
//! This module is only compiled when the `wgpu-backend` feature is active.

use crate::core::image_resource::{ImageResource, ImageResourceId};
use crate::core::render::RenderCommand;
use crate::core::render::RenderTarget as TurRenderTarget;
use crate::core::render::Renderer as TurRenderer;
use crate::core::render::{Surface, SurfaceHandle, downcast_surface};
use crate::renderer::vello::scene_paint::{new_scene, paint_commands_to_scene};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::collections::HashMap;
use vello_common::paint::{ImageId, ImageSource};
use vello_hybrid::{RenderSize, RenderTargetConfig, Renderer, Resources, Scene, TextureBindings};

#[derive(Debug, thiserror::Error)]
pub enum VelloRendererError {
    #[error("vello render failed: {0}")]
    Render(#[source] vello_hybrid::RenderError),
}

/// Opaque surface payload accepted by [`WgpuRenderer`] — a raw window +
/// display handle pair (winit window, Android `ANativeWindow`, …).
pub struct RawSurface {
    pub raw_display_handle: RawDisplayHandle,
    pub raw_window_handle: RawWindowHandle,
}

impl Surface for RawSurface {
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

/// Factory for [`VelloTarget`]s — owns the shared `wgpu::Instance` (behind
/// an `Arc` so multi-instance embedders can share one instance across
/// engine instances) so every view root's surface comes off one instance
/// (shared adapter discovery).
pub struct WgpuRenderer {
    instance: std::sync::Arc<wgpu::Instance>,
}

impl Default for WgpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl WgpuRenderer {
    /// Create with a fresh default instance (all backends).
    pub fn new() -> Self {
        Self::with_shared_instance(std::sync::Arc::new(wgpu::Instance::new(
            wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                flags: wgpu::InstanceFlags::default(),
                memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                backend_options: wgpu::BackendOptions::default(),
                display: None,
            },
        )))
    }

    /// Create wrapping an embedder-provided instance.
    pub fn with_instance(instance: wgpu::Instance) -> Self {
        Self::with_shared_instance(std::sync::Arc::new(instance))
    }

    /// Create sharing an embedder-provided instance `Arc` (multi-instance
    /// embedders share one wgpu instance across every engine instance).
    pub fn with_shared_instance(instance: std::sync::Arc<wgpu::Instance>) -> Self {
        Self { instance }
    }

    /// A shared handle to the underlying instance (embedders may pre-create
    /// surfaces / more factories with it).
    pub fn instance_handle(&self) -> std::sync::Arc<wgpu::Instance> {
        self.instance.clone()
    }
}

impl TurRenderer for WgpuRenderer {
    fn create_target(
        &mut self,
        surface: SurfaceHandle,
        viewport: (f64, f64),
        dpr: f64,
    ) -> Result<Box<dyn TurRenderTarget>, crate::error::TurError> {
        let raw = downcast_surface::<RawSurface>("WgpuRenderer", surface)?;
        let target = VelloTarget::init_raw(
            &self.instance,
            raw.raw_display_handle,
            raw.raw_window_handle,
            viewport.0 as u32,
            viewport.1 as u32,
            dpr,
        )
        .map_err(crate::error::TurError::Render)?;
        Ok(Box::new(target))
    }
}

/// One wgpu-backed vello-hybrid render target (a single window surface).
/// Created via [`WgpuRenderer::create_target`] or the direct
/// [`VelloTarget::init_surface`] constructor.
pub struct VelloTarget {
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

impl VelloTarget {
    #[allow(clippy::too_many_arguments)]
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

        VelloTarget {
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

    /// Create a target from raw window/display handles off the given
    /// instance: creates the surface, requests adapter + device + queue
    /// synchronously (`pollster`), then configures the target. The
    /// factory-path equivalent of [`Self::init_surface`].
    #[allow(clippy::too_many_arguments)]
    pub fn init_raw(
        instance: &wgpu::Instance,
        raw_display_handle: RawDisplayHandle,
        raw_window_handle: RawWindowHandle,
        logical_width: u32,
        logical_height: u32,
        dpr: f64,
    ) -> Result<Self, String> {
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(raw_display_handle),
                    raw_window_handle,
                })
                .map_err(|e| format!("create surface: {e}"))?
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(|e| format!("request adapter: {e}"))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|e| format!("request device: {e}"))?;
        Ok(Self::init_surface(
            &adapter,
            device,
            queue,
            surface,
            logical_width,
            logical_height,
            dpr,
        ))
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

    /// Render a flat command batch into the scene. Playback happens in
    /// `paint_commands_to_scene`; image upload happens incrementally via
    /// `TurRenderTarget::upload_image_resource` as the worker registers
    /// resources.
    pub fn render_commands_to_scene(&mut self, commands: &[RenderCommand]) {
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

    /// Upload one image resource to the hybrid image cache (atlas), caching
    /// its `ImageId` keyed by `ImageResourceId`. Called once per
    /// newly-registered resource (replaces the old per-frame full-map
    /// upload sweep).
    pub fn upload_image_resource(&mut self, id: ImageResourceId, image: &ImageResource) {
        if self.image_uploads.contains_key(&id) {
            return;
        }
        let source = ImageSource::from_peniko_image_data(&image.peniko_image);
        let pixmap = match source {
            ImageSource::Pixmap(p) => p,
            // Only inline pixmap sources are produced from decoded image data.
            _ => return,
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("image upload"),
            });
        let image_id = self.renderer.upload_image(
            &mut self.resources,
            &self.device,
            &self.queue,
            &mut encoder,
            &pixmap,
        );
        self.image_uploads.insert(id, image_id);
        self.queue.submit(std::iter::once(encoder.finish()));
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
                let (chunks, _) = data[offset..row_end].as_chunks::<4>();
                for chunk in chunks {
                    pixels.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
                }
            } else {
                pixels.extend_from_slice(&data[offset..row_end]);
            }
        }
        pixels
    }
}

impl TurRenderTarget for VelloTarget {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        // `physical_width` / `physical_height` / `dpr` are tracked on `self`
        // (kept in sync via `resize`, which fires on viewport-change events
        // only).
        self.render_commands_to_scene(commands);
    }

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        VelloTarget::present(self).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn resize(&mut self, logical_width: u32, logical_height: u32, dpr: f64) {
        VelloTarget::resize(self, logical_width, logical_height, dpr);
    }

    fn upload_image_resource(&mut self, id: ImageResourceId, image: &ImageResource) {
        VelloTarget::upload_image_resource(self, id, image);
    }

    fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        Some(VelloTarget::render_to_pixels(self))
    }
}
