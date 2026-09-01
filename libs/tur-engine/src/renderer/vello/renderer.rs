//! wgpu-backed vello-hybrid renderer (native + WebGPU targets).
//!
//! This module is only compiled when the `wgpu-backend` feature is active.
//!
//! ## wgpu error policy
//!
//! wgpu **reports** surface errors through the device error sink instead of
//! returning them (`Surface::configure` returns nothing at all), and its
//! default uncaptured-error handler panics. For an embedded engine a dead
//! window is a normal lifecycle event, not a fatal fault — so this renderer
//! installs a log-and-degrade handler on its device before its first
//! `configure`, and that is the whole policy: **no reported wgpu error
//! can abort the host process.** A failed `configure` simply leaves the
//! surface on its last-good config; `get_current_texture` then returns
//! `Lost`/`Outdated` (ordinary return values, handled by `present`), and the
//! engine loop keeps running — only the GPU output goes blank until the
//! embedder attaches a fresh surface.
//!
//! Surface *lifecycle* (never creating/configuring a surface for an instance
//! that is being torn down) is the embedder's job — see `tur-android`'s
//! two-phase initialize→attach model.

use crate::core::image_resource::{ImageResource, ImageResourceId};
use crate::core::render::RenderCommand;
use crate::core::render::Renderer as TurRenderer;
use crate::renderer::vello::scene_paint::{new_scene, paint_commands_to_scene};
use std::collections::HashMap;
use vello_common::paint::{ImageId, ImageSource};
use vello_hybrid::{RenderSize, RenderTargetConfig, Renderer, Resources, Scene, TextureBindings};

#[derive(Debug, thiserror::Error)]
pub enum VelloRendererError {
    #[error("vello render failed: {0}")]
    Render(#[source] vello_hybrid::RenderError),
    /// Surface initialization failed: the surface reports no
    /// capabilities/default config (its window is gone, or the adapter can't
    /// present to it). The embedder treats this like any other attach
    /// failure (log; the instance stays renderer-less and can attach again
    /// later), never a crash.
    #[error("surface init failed: {0}")]
    Init(String),
}

/// Install the renderer's uncaptured-error policy: log at ERROR and keep
/// running. Replaces wgpu's default handler, which panics — see the module
/// docs ("wgpu error policy"). Called from [`VelloRenderer::init_surface`]
/// before the first `configure` (the unit tests reach it via `super::`).
fn install_error_policy(device: &wgpu::Device) {
    device.on_uncaptured_error(std::sync::Arc::new(|err| {
        tracing::error!("uncaptured wgpu error (degraded, not fatal): {err:?}");
    }));
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
    ) -> Result<Self, VelloRendererError> {
        // Policy first: everything below may report errors through the sink,
        // and the default handler panics (see module docs).
        install_error_policy(&device);

        let max_texture_dimension = device.limits().max_texture_dimension_2d;
        let physical_width = ((logical_width as f64 * dpr) as u32).min(max_texture_dimension);
        let physical_height = ((logical_height as f64 * dpr) as u32).min(max_texture_dimension);

        // `get_default_config` returns None when the surface reports no
        // capabilities (dead/foreign window) — the one genuinely fallible
        // step, since a missing config can't be defaulted.
        let mut config = surface
            .get_default_config(adapter, physical_width, physical_height)
            .ok_or_else(|| {
                VelloRendererError::Init(
                    "surface capabilities unavailable (dead or unsupported surface)".into(),
                )
            })?;

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
                .or_else(|| caps.formats.first())
                .copied()
                // `get_default_config` returned Some ⇒ formats is non-empty,
                // so this is belt-and-braces; the default IS `formats[0]`.
                .unwrap_or(config.format)
        };
        config.format = surface_format;
        config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT;

        // A configure failure (dead window, zero-area, …) is *reported* to
        // the sink — logged by the policy above — and degrades: the surface
        // keeps its last-good config and `present`'s Lost/Outdated arms take
        // over. Never a panic, never a returned error to chase here.
        surface.configure(&device, &config);

        let render_target_config = RenderTargetConfig {
            format: surface_format,
            width: physical_width,
            height: physical_height,
        };
        let renderer = Renderer::new(&device, &render_target_config);

        let scene = new_scene(physical_width, physical_height);

        Ok(VelloRenderer {
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
        })
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
    /// `TurRenderer::upload_image_resource` as the worker registers
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
                // A failed reconfigure (dead window) is reported to the sink
                // and logged by the policy — see module docs.
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            other => {
                // `Lost` and anything else: warn and carry on — the engine
                // loop outlives its surface (see module docs).
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

impl TurRenderer for VelloRenderer {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        // `physical_width` / `physical_height` / `dpr` are tracked on `self`
        // (kept in sync via `resize`, which fires on viewport-change events
        // only).
        self.render_commands_to_scene(commands);
    }

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        VelloRenderer::present(self).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn resize(&mut self, logical_width: u32, logical_height: u32, dpr: f64) {
        VelloRenderer::resize(self, logical_width, logical_height, dpr);
    }

    fn upload_image_resource(&mut self, id: ImageResourceId, image: &ImageResource) {
        VelloRenderer::upload_image_resource(self, id, image);
    }

    fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        Some(VelloRenderer::render_to_pixels(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Request a headless adapter/device (no surface — these tests exercise
    /// the device error policy, not surface state). Returns `None` on
    /// machines with no usable adapter; the tests skip in that case.
    async fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok()?;
        adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .ok()
    }

    /// A deliberately-invalid WGSL source: `create_shader_module` reports
    /// the parse failure through the device error sink (a validation error,
    /// synchronously on native) and hands back an errored module.
    fn invalid_shader_module(device: &wgpu::Device) {
        let _ = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tur-test-deliberately-invalid"),
            source: wgpu::ShaderSource::Wgsl("this is not valid wgsl".into()),
        });
    }

    /// The renderer's error policy: an **uncaptured** wgpu error is logged
    /// and survived, not panicked on. wgpu's default uncaptured-error
    /// handler panics ("Handling wgpu errors as fatal by default") — the
    /// exact escalation that turned a dead-window surface configure into a
    /// process abort on Android. Reaching the end of this test without a
    /// panic IS the assertion.
    #[test]
    fn uncaptured_validation_error_degrades_instead_of_panicking() {
        let Some((device, _queue)) = pollster::block_on(headless_device()) else {
            eprintln!("skipping: no wgpu adapter available");
            return;
        };
        install_error_policy(&device);
        invalid_shader_module(&device);
        // Give any deferred error delivery a chance to land before the
        // device drops (native reports synchronously; this is belt+braces).
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
    }
}
