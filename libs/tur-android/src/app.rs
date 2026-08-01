//! tur Android engine wrapper: builds a shared [`TurRuntime`] once (fonts,
//! clock, capabilities, wgpu instance), then spawns isolated [`TurApp`]
//! instances — each attached to an Android `Surface` (rendering) or headless
//! (no rendering). Exposes the operations the JNI layer drives.
//!
//! On non-Android targets the crate compiles as a stub so the workspace builds
//! on desktop; this module is then empty.

#[cfg(target_os = "android")]
mod imp {
    use std::rc::Rc;

    use boa_engine::context::time::StdClock;
    use jni::objects::GlobalRef;
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
    use tur_clipboard_android::{AndroidClipboard, Clipboard};
    use tur_engine::error::TurError;
    use tur_engine::renderer::vello::VelloRenderer;
    use tur_engine::{CursorCap, NoopCursor, TurApp, TurRuntime, TurRuntimeBuilder};
    use tur_net_native::{Http, NativeHttp};

    use crate::loop_driver::{AndroidLoopDriver, FrameLoopRef};
    use crate::surface::AndroidWindowHandle;
    use tur_native::NativeFontLoader;

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

    /// The shared runtime — created once per app process. Owns the
    /// [`TurRuntime`] (fonts, clock, capabilities, plugins) plus a single
    /// shared `wgpu::Instance` that every rendering instance creates its
    /// `Surface` from. Returned to the JNI layer as a boxed pointer (the
    /// `jlong` runtime handle Kotlin holds).
    pub struct AndroidRuntime {
        pub runtime: Rc<TurRuntime>,
        pub wgpu_instance: wgpu::Instance,
    }

    impl AndroidRuntime {
        /// Build the shared runtime. `configure` receives the
        /// [`TurRuntimeBuilder`] (by value) AFTER the Android defaults are
        /// installed (native font loader, wall-clock `StdClock`, `NoopCursor`,
        /// `AndroidClipboard`, `NativeHttp`), so the callback only needs to
        /// chain `.plugin(…)` calls and return the builder.
        pub fn build(
            context: GlobalRef,
            configure: impl FnOnce(TurRuntimeBuilder) -> TurRuntimeBuilder,
        ) -> Result<Self, TurAndroidError> {
            // Register the process JavaVM for the clipboard backend (it attaches
            // per call to reach ClipboardManager).
            tur_clipboard_android::set_java_vm(crate::java_vm().expect("JavaVM set before create"));

            let mut builder = TurRuntime::builder()
                .font_loader(Rc::new(NativeFontLoader::new()))
                .clock(Rc::new(StdClock::new()))
                .capability(CursorCap::new(NoopCursor))
                .capability(Clipboard::new(AndroidClipboard::new(context)))
                .capability(Http::new(NativeHttp::default()));
            builder = configure(builder);
            let runtime = builder.build()?;

            let wgpu_instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::VULKAN,
                flags: wgpu::InstanceFlags::default(),
                memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                backend_options: wgpu::BackendOptions::default(),
                display: None,
            });

            Ok(Self {
                runtime,
                wgpu_instance,
            })
        }
    }

    /// One isolated engine instance — either rendering (attached to a Surface)
    /// or headless. Built from an [`AndroidRuntime`] via
    /// [`AndroidInstance::build_with_surface`] or
    /// [`AndroidInstance::build_headless`]. Returned to the JNI layer as a
    /// boxed pointer (the `jlong` instance handle Kotlin holds).
    pub struct AndroidInstance {
        pub app: Rc<TurApp>,
        pub loop_driver: Rc<AndroidLoopDriver>,
    }

    impl AndroidInstance {
        /// Build a rendering instance over a freshly-created wgpu surface
        /// backed by the given Android `Surface`'s `ANativeWindow*`, using the
        /// runtime's shared `wgpu::Instance`. `frame_loop` drives the wake
        /// cadence.
        pub async fn build_with_surface(
            runtime: &Rc<TurRuntime>,
            wgpu_instance: &wgpu::Instance,
            window_handle: AndroidWindowHandle,
            logical_width: u32,
            logical_height: u32,
            dpr: f64,
            frame_loop: FrameLoopRef,
        ) -> Result<Self, TurAndroidError> {
            let raw_display = window_handle
                .display_handle()
                .map_err(|e| TurAndroidError::WgpuSurface(format!("display: {e}")))?;
            let raw_window = window_handle
                .window_handle()
                .map_err(|e| TurAndroidError::WgpuSurface(format!("window: {e}")))?;

            let surface = unsafe {
                wgpu_instance
                    .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: Some(raw_display.as_raw()),
                        raw_window_handle: raw_window.as_raw(),
                    })
                    .map_err(|e| TurAndroidError::WgpuSurface(e.to_string()))?
            };

            let adapter = wgpu_instance
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

            let app = runtime.create_app(
                Box::new(renderer),
                (logical_width as f64, logical_height as f64),
                dpr,
            )?;

            let loop_driver = Rc::new(AndroidLoopDriver::new(frame_loop));
            app.start(loop_driver.clone());

            Ok(Self { app, loop_driver })
        }

        /// Build a headless instance (no surface, no rendering) from the
        /// runtime. Runs JS + capabilities + events only.
        pub fn build_headless(
            runtime: &Rc<TurRuntime>,
            frame_loop: FrameLoopRef,
        ) -> Result<Self, TurAndroidError> {
            let app = runtime.create_headless_app((0.0, 0.0))?;
            let loop_driver = Rc::new(AndroidLoopDriver::new(frame_loop));
            app.start(loop_driver.clone());
            Ok(Self { app, loop_driver })
        }
    }
}

#[cfg(target_os = "android")]
#[allow(unused_imports)]
pub use imp::{AndroidInstance, AndroidRuntime, TurAndroidError};

#[cfg(not(target_os = "android"))]
mod imp {
    use crate::loop_driver::FrameLoopRef;
    use crate::surface::AndroidWindowHandle;
    use jni::objects::GlobalRef;
    use tur_engine::TurRuntimeBuilder;

    // Stub so the crate type-checks on desktop. Never constructed at runtime.
    #[derive(Debug, thiserror::Error)]
    pub enum TurAndroidError {
        #[error("android-only")]
        AndroidOnly,
    }

    pub struct AndroidRuntime;

    impl AndroidRuntime {
        pub fn build(
            _context: GlobalRef,
            _configure: impl FnOnce(TurRuntimeBuilder) -> TurRuntimeBuilder,
        ) -> Result<Self, TurAndroidError> {
            Err(TurAndroidError::AndroidOnly)
        }
    }

    pub struct AndroidInstance;

    impl AndroidInstance {
        pub async fn build_with_surface(
            _runtime: &std::rc::Rc<tur_engine::TurRuntime>,
            _wgpu_instance: &wgpu::Instance,
            _window_handle: AndroidWindowHandle,
            _logical_width: u32,
            _logical_height: u32,
            _dpr: f64,
            _frame_loop: FrameLoopRef,
        ) -> Result<Self, TurAndroidError> {
            Err(TurAndroidError::AndroidOnly)
        }

        pub fn build_headless(
            _runtime: &std::rc::Rc<tur_engine::TurRuntime>,
            _frame_loop: FrameLoopRef,
        ) -> Result<Self, TurAndroidError> {
            Err(TurAndroidError::AndroidOnly)
        }
    }
}

#[cfg(not(target_os = "android"))]
#[allow(unused_imports)]
pub use imp::{AndroidInstance, AndroidRuntime, TurAndroidError};
