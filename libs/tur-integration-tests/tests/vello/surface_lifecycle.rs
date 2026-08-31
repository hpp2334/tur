//! Surface-lifecycle degradation tests: a surface that cannot be configured
//! (dead window, zero-area) must degrade the renderer — return an error from
//! `init_surface`, or suppress surface work after a failed `resize` — never
//! panic. These pin the Android young-instance teardown race: wgpu *reports*
//! configure errors through the device error sink, and its default handler
//! panics (→ SIGABRT on the tur-host thread).

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tur_engine::renderer::vello::VelloRenderer;

use crate::vello_app::TurVelloApp;

/// Build a real window + wgpu surface/adapter/device (mirroring
/// `vello_app::init_async`) for direct `init_surface` experiments.
async fn surface_stack(
    width: usize,
    height: usize,
) -> (
    minifb::Window,
    wgpu::Adapter,
    wgpu::Device,
    wgpu::Queue,
    wgpu::Surface<'static>,
) {
    let window = minifb::Window::new(
        "tur-vello-surface-lifecycle",
        width,
        height,
        minifb::WindowOptions {
            resize: false,
            ..Default::default()
        },
    )
    .expect("window creation");

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        flags: wgpu::InstanceFlags::default(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        backend_options: wgpu::BackendOptions::default(),
        display: None,
    });

    let raw_display = window.display_handle().expect("display handle");
    let raw_window = window.window_handle().expect("window handle");
    let surface = unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display.as_raw()),
                raw_window_handle: raw_window.as_raw(),
            })
            .expect("surface creation")
    };

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .expect("adapter request");
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .expect("device request");

    (window, adapter, device, queue, surface)
}

/// A zero-area surface is the deterministic stand-in for a dead window: the
/// capabilities are fine but `configure` raises `ConfigureSurfaceError`
/// (ZeroArea), which wgpu *reports* rather than returns. The renderer's
/// error policy (module docs) turns that into a logged, degraded surface —
/// `init_surface` returns `Ok` (the renderer exists; the surface simply
/// never presents) and nothing panics. Pre-policy, the uncaptured error hit
/// wgpu's default handler and aborted the process.
pub fn init_surface_zero_area_degrades() {
    let (_window, adapter, device, queue, surface) = pollster::block_on(surface_stack(64, 64));
    let renderer = VelloRenderer::init_surface(&adapter, device, queue, surface, 0, 0, 1.0);
    assert!(
        renderer.is_ok(),
        "zero-area init_surface must degrade to Ok (configure error logged), got: {:?}",
        renderer.err()
    );
}

/// Resize through a failing configure (zero area) must degrade — not panic —
/// and a subsequent valid resize recovers the surface.
pub fn resize_zero_area_degrades_and_recovers() {
    let app = TurVelloApp::new(96.0, 96.0, 1.0).expect("harness build");

    // Degrade: configure a zero-area size (the deterministic dead-window
    // stand-in). The failed configure suppresses surface work; driving a
    // frame (present path) must not panic.
    app.app().resize(0, 0, 1.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Recover: a valid size reconfigures (the retry-on-resize path clears
    // the suppression) and rendering continues.
    app.app().resize(96, 96, 1.0);
    app.wait_for_timeout(std::time::Duration::ZERO);

    // The renderer still produces frames — a full-size readback (the
    // offscreen path renders through the same device + scene state a
    // broken renderer would have poisoned; pre-fix, the zero-area resize
    // panicked before any of this).
    let pixels = app.render_to_pixels();
    assert_eq!(
        pixels.len(),
        96 * 96 * 4,
        "renderer recovered after zero-area resize"
    );
}
