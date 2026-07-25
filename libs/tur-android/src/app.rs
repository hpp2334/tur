//! tur Android engine wrapper: builds the [`TurApp`] with all plugins and a
//! wgpu/Vulkan renderer over an Android `Surface`, and exposes the operations
//! the JNI layer drives (load JS, push events, pump a frame, attach a surface).
//!
//! Mirrors the boot sequence in `vello_app.rs` (the native test harness) — only
//! the surface source (Android `ANativeWindow` vs minifb window) and event
//! source (JNI vs winit/minifb) differ.
//!
//! On non-Android targets the crate compiles as a stub so the workspace builds
//! on desktop; this module is then empty.

#[cfg(target_os = "android")]
mod imp {
    use std::rc::Rc;

    use boa_engine::context::time::StdClock;
    use jni::objects::GlobalRef;
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
    use tur_animation::TurAnimationPlugin;
    use tur_clipboard_android::{AndroidClipboard, Clipboard};
    use tur_demo_plugin::TurDemoPlugin;
    use tur_engine::core::platform::PlatformEvent;
    use tur_engine::error::TurError;
    use tur_engine::renderer::vello::VelloRenderer;
    use tur_engine::{CursorCap, NoopCursor, TurApp, TurEngine, TurStdPlugin};
    use tur_net_native::{Http, NativeHttp, TurNetPlugin};

    use tur_native::NativeFontLoader;
    use crate::loop_driver::{AndroidLoopDriver, FrameLoopRef};
    use crate::surface::AndroidWindowHandle;

    #[derive(Debug, thiserror::Error)]
    pub enum TurAndroidError {
        #[error(transparent)]
        Engine(#[from] TurError),
        #[error("wgpu adapter request failed: {0}")]
        WgpuAdapter(String),
        #[error("wgpu device request failed: {0}")]
        WgpuDevice(String),
        #[error("wgpu surface creation failed: {0}")]
        WgpuSurface(String),
    }

    /// The built engine + its Android `LoopDriver`. Returned to the JNI layer
    /// as a boxed pointer (the `jlong` handle Kotlin holds).
    pub struct AndroidApp {
        pub app: Rc<TurApp>,
        pub loop_driver: Rc<AndroidLoopDriver>,
    }

    impl AndroidApp {
        /// Build the engine over a freshly-created wgpu surface backed by the
        /// given Android `Surface`'s `ANativeWindow*`. `frame_loop` is a JNI
        /// global ref to Kotlin's `ai.tur.FrameLoop` (drives the wake cadence).
        pub fn create(
            context: GlobalRef,
            window_handle: AndroidWindowHandle,
            logical_width: u32,
            logical_height: u32,
            dpr: f64,
            frame_loop: FrameLoopRef,
        ) -> Result<Self, TurAndroidError> {
            // Register the process JavaVM for the clipboard backend (it attaches
            // per call to reach ClipboardManager).
            tur_clipboard_android::set_java_vm(
                crate::java_vm().expect("JavaVM set before create"),
            );

            let (app, loop_driver) = pollster::block_on(Self::init_async(
                context,
                window_handle,
                logical_width,
                logical_height,
                dpr,
                frame_loop,
            ))?;
            Ok(Self { app, loop_driver })
        }

        async fn init_async(
            context: GlobalRef,
            window_handle: AndroidWindowHandle,
            logical_width: u32,
            logical_height: u32,
            dpr: f64,
            frame_loop: FrameLoopRef,
        ) -> Result<(Rc<TurApp>, Rc<AndroidLoopDriver>), TurAndroidError> {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::VULKAN,
                flags: wgpu::InstanceFlags::default(),
                memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                backend_options: wgpu::BackendOptions::default(),
                display: None,
            });

            let raw_display = window_handle
                .display_handle()
                .map_err(|e| TurAndroidError::WgpuSurface(format!("display: {e}")))?;
            let raw_window = window_handle
                .window_handle()
                .map_err(|e| TurAndroidError::WgpuSurface(format!("window: {e}")))?;

            let surface = unsafe {
                instance
                    .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: Some(raw_display.as_raw()),
                        raw_window_handle: raw_window.as_raw(),
                    })
                    .map_err(|e| TurAndroidError::WgpuSurface(e.to_string()))?
            };

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .map_err(|e| TurAndroidError::WgpuAdapter(e.to_string()))?;

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .map_err(|e| TurAndroidError::WgpuDevice(e.to_string()))?;

            let renderer = VelloRenderer::init_surface(
                &adapter,
                device,
                queue,
                surface,
                logical_width,
                logical_height,
                dpr,
            );

            let app = TurEngine::builder()
                .renderer(Box::new(renderer))
                .font_loader(Box::new(NativeFontLoader::new()))
                .clock(Rc::new(StdClock::new()))
                .capability(CursorCap::new(NoopCursor))
                .capability(Clipboard::new(AndroidClipboard::new(context)))
                .capability(Http::new(NativeHttp::default()))
                .plugin(TurStdPlugin)
                .plugin(TurAnimationPlugin)
                .plugin(tur_engine::TurClipboardPlugin)
                .plugin(TurNetPlugin)
                .plugin(TurDemoPlugin)
                .build()?;

            app.push_platform_event(PlatformEvent::Resize {
                logical_width,
                logical_height,
                dpr,
            });

            // Install the Android loop driver (Choreographer-backed) and run
            // frame 1 (which processes the resize above), then arm follow-up
            // wake-ups per the engine's verdict.
            let loop_driver = Rc::new(AndroidLoopDriver::new(frame_loop));
            app.start(loop_driver.clone());

            Ok((app, loop_driver))
        }
    }
}

#[cfg(target_os = "android")]
#[allow(unused_imports)]
pub use imp::{AndroidApp, TurAndroidError};

#[cfg(not(target_os = "android"))]
mod imp {
    use jni::objects::GlobalRef;
    use crate::loop_driver::FrameLoopRef;
    use crate::surface::AndroidWindowHandle;

    // Stub so the crate type-checks on desktop. Never constructed at runtime.
    #[derive(Debug, thiserror::Error)]
    pub enum TurAndroidError {
        #[error("android-only")]
        AndroidOnly,
    }

    pub struct AndroidApp;

    impl AndroidApp {
        pub fn create(
            _context: GlobalRef,
            _window_handle: AndroidWindowHandle,
            _logical_width: u32,
            _logical_height: u32,
            _dpr: f64,
            _frame_loop: FrameLoopRef,
        ) -> Result<Self, TurAndroidError> {
            Err(TurAndroidError::AndroidOnly)
        }
    }
}

#[cfg(not(target_os = "android"))]
#[allow(unused_imports)]
pub use imp::{AndroidApp, TurAndroidError};