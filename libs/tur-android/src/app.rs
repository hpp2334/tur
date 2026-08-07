//! tur Android engine wrapper: builds a shared [`TurRuntime`] once (fonts,
//! clock, capabilities, wgpu instance), then spawns isolated [`TurApp`]
//! instances — each attached to an Android `Surface` (rendering) or headless
//! (no rendering). Exposes the operations the JNI layer drives.
//!
//! On non-Android targets the crate compiles as a stub so the workspace builds
//! on desktop; this module is then empty.

#[cfg(target_os = "android")]
mod imp {
    use std::pin::Pin;
    use std::rc::Rc;

    use boa_engine::context::time::StdClock;
    use jni::objects::GlobalRef;
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
    use tur_clipboard_android::{AndroidClipboard, Clipboard};
    use tur_engine::error::TurError;
    use tur_engine::renderer::vello::VelloRenderer;
    use tur_engine::{CursorCap, NoopCursor, TurApp, TurRuntime, TurRuntimeBuilder};

    use crate::scheduler::{AndroidSchedulerDriver, FrameLoopRef};
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
    /// [`TurRuntime`] (fonts, clock, capabilities, plugins), a shared
    /// `wgpu::Instance` that every rendering instance creates its `Surface`
    /// from, and the tokio runtime whose handle backs the scheduler's
    /// `sleep` timers. Returned to the JNI layer as a boxed pointer (the
    /// `jlong` runtime handle Kotlin holds).
    pub struct AndroidRuntime {
        pub runtime: Rc<TurRuntime>,
        pub wgpu_instance: wgpu::Instance,
        /// Tokio runtime — `sleep` timers (via
        /// [`AndroidSchedulerDriver`]) + optionally `NativeHttp`/reqwest.
        pub tokio: tokio::runtime::Runtime,
    }

    impl AndroidRuntime {
        /// Build the shared runtime. `configure` receives the
        /// [`TurRuntimeBuilder`] (by value) AFTER the Android defaults are
        /// installed (native font loader, wall-clock `StdClock`, `NoopCursor`,
        /// `AndroidClipboard`, the base scheduler driver), so the callback
        /// only needs to chain `.plugin(…)` calls (and, if HTTP is wanted,
        /// register a `NativeHttp` backend against the same tokio runtime
        /// via [`Self::tokio_handle`]) and return the builder.
        pub fn build(
            context: GlobalRef,
            configure: impl FnOnce(TurRuntimeBuilder) -> TurRuntimeBuilder,
        ) -> Result<Self, TurAndroidError> {
            // Register the process JavaVM for the clipboard backend (it attaches
            // per call to reach ClipboardManager).
            tur_clipboard_android::set_java_vm(crate::java_vm().expect("JavaVM set before create"));

            // Timers for scheduler `sleep`. Also shared with `NativeHttp`.
            let tokio = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_time()
                .build()
                .map_err(|e| TurAndroidError::Engine(TurError::Other(format!("tokio: {e}"))))?;

            // Base scheduler: no frame loop (that's per-instance — each
            // instance installs its own via `TurApp::set_main_scheduler`).
            let driver = AndroidSchedulerDriver::new(tokio.handle().clone(), None);

            let mut builder = TurRuntime::builder()
                .scheduler(driver)
                .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
                .clock(std::sync::Arc::new(StdClock::new()))
                .capability(|_| Ok(CursorCap::new(NoopCursor)))
                .capability(move |_| Ok(Clipboard::new(AndroidClipboard::new(context))));
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
                tokio,
            })
        }

        /// Handle to the runtime's tokio runtime — embedders can register a
        /// `NativeHttp` backend against it in `configure`.
        pub fn tokio_handle(&self) -> tokio::runtime::Handle {
            self.tokio.handle().clone()
        }
    }

    /// One isolated engine instance — either rendering (attached to a Surface)
    /// or headless. Built from an [`AndroidRuntime`] via
    /// [`AndroidInstance::build_with_surface`] or
    /// [`AndroidInstance::build_headless`]. Returned to the JNI layer as a
    /// boxed pointer (the `jlong` instance handle Kotlin holds).
    pub struct AndroidInstance {
        pub app: Rc<TurApp>,
        pub scheduler: Rc<AndroidSchedulerDriver>,
        /// The autonomous `start_loop` future. `TurApp` is `Rc`-based
        /// (!Send — the boa realm lives on the main thread), so the loop
        /// cannot run on a spawned thread: JNI `pump` polls it once per
        /// Choreographer tick.
        loop_task: std::cell::RefCell<Option<Pin<Box<dyn Future<Output = ()>>>>>,
    }

    impl AndroidInstance {
        /// Install the per-instance scheduler (bound to this instance's
        /// Kotlin `FrameLoop`) and stash the autonomous `start_loop` future
        /// for poll-per-`pump` driving. Arms the first vsync so the
        /// bootstrap `FrameOutcome` (from `create_app`'s initial resize)
        /// kicks the loop off.
        fn install_frame_loop(
            app: &Rc<TurApp>,
            frame_loop: FrameLoopRef,
            tokio: &tokio::runtime::Handle,
        ) -> (
            Rc<AndroidSchedulerDriver>,
            std::cell::RefCell<Option<Pin<Box<dyn Future<Output = ()>>>>>,
        ) {
            use tur_engine::core::scheduler::MainScheduler;

            let driver = AndroidSchedulerDriver::new(tokio.clone(), Some(frame_loop));
            app.set_main_scheduler(MainScheduler::new(driver.clone()));
            // Bootstrap: arm the first Choreographer callback. Subsequent
            // frames re-arm via the loop's `request_vsync` on `FrameOutcome`.
            driver.request_vsync();
            let loop_task = std::cell::RefCell::new(Some(
                Box::pin(app.clone().start_loop()) as Pin<Box<dyn Future<Output = ()>>>
            ));
            (driver, loop_task)
        }

        /// Poll the autonomous loop exactly once. Called from JNI `pump`
        /// after `fire_vsync`; each poll handles at most one vsync/main-msg
        /// event, so the loop is pulled forward by the Choreographer
        /// cadence.
        pub fn pump_loop(&self) {
            let mut task = self.loop_task.borrow_mut();
            let ready = task
                .as_mut()
                .map(|t| {
                    let waker = futures::task::noop_waker();
                    let mut cx = std::task::Context::from_waker(&waker);
                    t.as_mut().poll(&mut cx)
                })
                .map_or(false, |p| p.is_ready());
            if ready {
                *task = None;
            }
        }
        /// Build a rendering instance over a freshly-created wgpu surface
        /// backed by the given Android `Surface`'s `ANativeWindow*`, using the
        /// runtime's shared `wgpu::Instance`. `frame_loop` drives the wake
        /// cadence.
        ///
        /// Architecture: the engine runs on a worker thread; `MainBackend`
        /// owns the wgpu `VelloRenderer` on the caller thread (main) and
        /// drives it directly — command batches, incremental image uploads,
        /// and resize-on-event.
        #[allow(clippy::too_many_arguments)]
        pub async fn build_with_surface(
            runtime: &Rc<TurRuntime>,
            tokio: &tokio::runtime::Handle,
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

            // Engine on worker; `MainBackend` owns the wgpu renderer on
            // main and drives it directly (render batches, image uploads,
            // resize-on-event) — no render_sink callback.
            let app = runtime.create_app(
                Box::new(renderer),
                (logical_width as f64, logical_height as f64),
                dpr,
            )?;

            let (scheduler, loop_task) = Self::install_frame_loop(&app, frame_loop, tokio);

            Ok(Self {
                app,
                scheduler,
                loop_task,
            })
        }

        /// Build a headless instance (no surface, no rendering) from the
        /// runtime. Runs JS + capabilities + events only.
        pub fn build_headless(
            runtime: &Rc<TurRuntime>,
            tokio: &tokio::runtime::Handle,
            frame_loop: FrameLoopRef,
        ) -> Result<Self, TurAndroidError> {
            let app = runtime.create_app(
                Box::new(tur_engine::renderer::noop::NoopRenderer::new()),
                (0.0, 0.0),
                1.0,
            )?;
            let (scheduler, loop_task) = Self::install_frame_loop(&app, frame_loop, tokio);

            Ok(Self {
                app,
                scheduler,
                loop_task,
            })
        }
    }
}

#[cfg(target_os = "android")]
#[allow(unused_imports)]
pub use imp::{AndroidInstance, AndroidRuntime, TurAndroidError};

#[cfg(not(target_os = "android"))]
mod imp {
    use crate::scheduler::FrameLoopRef;
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
        #[allow(clippy::too_many_arguments)]
        pub async fn build_with_surface(
            _runtime: &std::rc::Rc<tur_engine::TurRuntime>,
            _tokio: &tokio::runtime::Handle,
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
            _tokio: &tokio::runtime::Handle,
            _frame_loop: FrameLoopRef,
        ) -> Result<Self, TurAndroidError> {
            Err(TurAndroidError::AndroidOnly)
        }
    }
}

#[cfg(not(target_os = "android"))]
#[allow(unused_imports)]
pub use imp::{AndroidInstance, AndroidRuntime, TurAndroidError};
