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
    use jni::objects::{GlobalRef, JObject, JValue};
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
    use tur_clipboard_android::{AndroidClipboard, Clipboard};
    use tur_engine::core::scheduler::{VsyncSource, WorkerPoolHandle};
    use tur_engine::error::TurError;
    use tur_engine::renderer::vello::VelloRenderer;
    use tur_engine::{
        CursorCap, FocusedState, NoopCursor, TurApp, TurAppBuilder, TurRuntime, TurRuntimeBuilder,
    };
    use tur_net_native::{Http, NativeHttp};

    /// `std::task::Wake` impl wrapping a `Send + Sync` closure that
    /// requests a main-loop poll. Used as the waker for `pump_loop` so that
    /// when the worker sends to `main_rx`, the channel fires the waker →
    /// coalesced Handler post on the main thread → `pump_loop` → drains
    /// the message. (Deliberately NOT a Choreographer arm — see
    /// [`AndroidVsyncSource::make_vsync_wake_fn`].)
    struct VsyncWaker(std::sync::Arc<dyn Fn() + Send + Sync>);

    impl std::task::Wake for VsyncWaker {
        fn wake(self: std::sync::Arc<Self>) {
            (self.0)();
        }
    }

    use crate::scheduler::{AndroidMainLoop, AndroidVsyncSource, FrameLoopRef};
    use crate::surface::AndroidWindowHandle;
    use tur_native::NativeFontLoader;

    /// Push the engine's focused-element editable flag to Kotlin's
    /// `FrameLoop.onFocusChanged(boolean)` via JNI. Called from the engine's
    /// focus-change handler (installed in [`AndroidInstance::install_frame_loop`])
    /// on the main thread, so `attach_current_thread` is a cheap no-op (the
    /// main looper thread is already attached). Kotlin retains the value so
    /// the per-frame `syncIme` poll reads it without a JNI round-trip.
    fn push_focus_to_kotlin(frame_loop: &FrameLoopRef, is_editable: bool) {
        let Some(vm) = crate::java_vm() else {
            return;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return;
        };
        let loop_obj = unsafe { JObject::from_raw(frame_loop.kotlin_loop.as_raw()) };
        let _ = env.call_method(
            &loop_obj,
            "onFocusChanged",
            "(Z)V",
            &[JValue::Bool(is_editable as jni::sys::jboolean)],
        );
    }

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
    /// from, and the tokio runtime whose handle backs the lane timers,
    /// `sleep` timers. Returned to the JNI layer as a boxed pointer (the
    /// `jlong` runtime handle Kotlin holds).
    pub struct AndroidRuntime {
        pub runtime: Rc<TurRuntime>,
        pub wgpu_instance: wgpu::Instance,
        /// Tokio runtime — lane `sleep` timers (via
        /// [`crate::scheduler::TokioLaneTimer`]) + optionally
        /// `NativeHttp`/reqwest.
        pub tokio: tokio::runtime::Runtime,
        /// The runtime's main-thread task spawner. Each instance registers
        /// its vsync-arm closure (so pending main-loop tasks schedule a
        /// Choreographer tick) and polls the tasks from `pump_loop`.
        pub main_loop: Rc<AndroidMainLoop>,
        /// The default worker pool every instance is assigned to unless the
        /// embedder overrides via `configure`. Effectively uncapped → each
        /// instance gets its own dedicated lane thread (the historical
        /// threading). Register additional capped pools on the builder
        /// (e.g. a small `daemon` pool for headless background instances)
        /// and assign them via `configure_instance`.
        pub default_worker_pool: WorkerPoolHandle,
    }

    impl AndroidRuntime {
        /// Build the shared runtime. `configure` receives the
        /// [`TurRuntimeBuilder`] (by value) AFTER the Android defaults are
        /// installed (native font loader, wall-clock `StdClock`, `NoopCursor`,
        /// `AndroidClipboard`, `NativeHttp`, the base scheduler driver), so the
        /// callback only needs to chain `.plugin(…)` calls and return the
        /// builder.
        pub fn build(
            context: GlobalRef,
            configure: impl FnOnce(TurRuntimeBuilder) -> TurRuntimeBuilder,
        ) -> Result<Self, TurAndroidError> {
            // Register the process JavaVM for the clipboard backend (it attaches
            // per call to reach ClipboardManager).
            tur_clipboard_android::set_java_vm(crate::java_vm().expect("JavaVM set before create"));

            // Timers for scheduler `sleep` + IO for `NativeHttp` (reqwest TCP).
            let tokio = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_time()
                .enable_io()
                .build()
                .map_err(|e| TurAndroidError::Engine(TurError::Other(format!("tokio: {e}"))))?;

            // Scheduling: worker hosting on native lane pools (tokio
            // timers); base vsync with no frame loop (per-instance — each
            // instance installs its own via `TurApp::set_vsync_source`);
            // main-loop tasks polled from each instance's `pump_loop`.
            let worker_host = crate::scheduler::worker_host(tokio.handle().clone());
            let base_vsync = AndroidVsyncSource::new(None);
            let main_loop = AndroidMainLoop::new();

            // Default (effectively uncapped) worker pool — one dedicated
            // lane thread per instance unless the embedder registers more.
            let default_worker_pool = WorkerPoolHandle::new("default", usize::MAX);

            let mut builder = TurRuntime::builder()
                .worker_host(worker_host)
                .vsync_source(base_vsync)
                .main_loop(main_loop.clone())
                .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
                .clock(std::sync::Arc::new(StdClock::new()))
                .worker_pool(default_worker_pool.clone())
                .capability(|_| Ok(CursorCap::new(NoopCursor)))
                .capability(move |_| Ok(Clipboard::new(AndroidClipboard::new(context))))
                .capability({
                    let handle = tokio.handle().clone();
                    move |_| Ok(Http::new(NativeHttp::new(handle.clone())))
                });
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
                main_loop,
                default_worker_pool,
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
        /// Per-instance vsync source bound to this instance's Kotlin
        /// `FrameLoop`. JNI `pump` fires it before polling the loop.
        pub vsync: Rc<AndroidVsyncSource>,
        /// The runtime's main-thread task spawner, polled each `pump`.
        main_loop: Rc<AndroidMainLoop>,
        /// The autonomous `run_loop` future. `TurApp` is `Rc`-based
        /// (!Send — the boa realm lives on the main thread), so the loop
        /// cannot run on a spawned thread: JNI `pump` polls it once per
        /// Choreographer tick.
        loop_task: std::cell::RefCell<Option<Pin<Box<dyn Future<Output = ()>>>>>,
        /// `Send + Sync` closure that requests a main-loop pump. Used
        /// as the waker for `pump_loop` so that when the worker sends to
        /// `main_rx` (from its own thread), the channel waker fires this
        /// closure → coalesced Handler post → `pump_loop` runs → processes
        /// the message. Without this the loop used `noop_waker` and worker
        /// messages sat unconsumed until the next input event.
        vsync_wake_fn: std::sync::Arc<dyn Fn() + Send + Sync>,
    }

    impl AndroidInstance {
        /// Install the per-instance vsync source (bound to this instance's
        /// Kotlin `FrameLoop`), the focus-change handler (pushes focused-
        /// element state into Kotlin so the per-frame IME sync can read it
        /// without a JNI round-trip), and stash the autonomous `run_loop`
        /// future for poll-per-`pump` driving. Arms the first vsync so the
        /// bootstrap `FrameOutcome` (from `app_builder().build(...)`'s
        /// initial resize) kicks the loop off. Also registers the message
        /// wake fn with the runtime's main loop so pending main-thread
        /// tasks (the engine's drain) get a prompt pump.
        fn install_frame_loop(
            app: &Rc<TurApp>,
            frame_loop: FrameLoopRef,
            main_loop: &Rc<AndroidMainLoop>,
        ) -> (
            Rc<AndroidVsyncSource>,
            std::cell::RefCell<Option<Pin<Box<dyn Future<Output = ()>>>>>,
            std::sync::Arc<dyn Fn() + Send + Sync>,
        ) {
            // Clone the FrameLoopRef before it's moved into the source —
            // the focus handler captures one to call `FrameLoop.onFocusChanged`
            // via JNI. The handler runs inside `apply_msg` on the main thread
            // (the JNI thread), so `attach_current_thread` is a cheap no-op
            // there, mirroring the source's `request_frame` path.
            let frame_loop_for_focus = frame_loop.clone();
            app.set_focus_changed_handler(Some(Rc::new(move |state: FocusedState| {
                push_focus_to_kotlin(&frame_loop_for_focus, state.is_editable);
            })));

            let vsync = AndroidVsyncSource::new(Some(frame_loop));
            app.set_vsync_source(vsync.clone());
            // Bootstrap: arm the first Choreographer callback. Subsequent
            // frames re-arm via the engine's `request_frame` on a
            // `FrameOutcome { schedule: Vsync }`.
            vsync.request_frame();
            let vsync_wake_fn = vsync.make_vsync_wake_fn();
            // Pending main-loop tasks (the engine's drain) request a pump on
            // this instance → the next `pump_loop` polls them.
            main_loop.add_wake_fn(vsync_wake_fn.clone());
            let loop_task = std::cell::RefCell::new(Some(
                Box::pin(app.clone().run_loop()) as Pin<Box<dyn Future<Output = ()>>>
            ));
            (vsync, loop_task, vsync_wake_fn)
        }

        /// Poll the autonomous loop exactly once. Called from JNI `pump`
        /// (Choreographer-fired: vsync + poll) and `pumpMessages`
        /// (message-pump: poll only). Each poll handles at most one
        /// vsync/main-msg event, so the loop is pulled forward by whichever
        /// cadence is active — display frames while animating, one Handler
        /// post per message batch while idle.
        ///
        /// Uses a **real waker** (not `noop_waker`) backed by
        /// [`vsync_wake_fn`](Self::vsync_wake_fn): when the worker sends to
        /// `main_rx`, the channel waker fires the closure → coalesced
        /// Handler post → this method runs again → processes the message.
        /// Without this, worker messages (render batches, frame outcomes)
        /// would sit unconsumed between input events.
        pub fn pump_loop(&self) {
            {
                let mut task = self.loop_task.borrow_mut();
                let ready = task
                    .as_mut()
                    .map(|t| {
                        let waker = std::task::Waker::from(std::sync::Arc::new(VsyncWaker(
                            self.vsync_wake_fn.clone(),
                        )));
                        let mut cx = std::task::Context::from_waker(&waker);
                        t.as_mut().poll(&mut cx)
                    })
                    .map_or(false, |p| p.is_ready());
                if ready {
                    *task = None;
                }
            }
            // Advance the runtime's main-thread tasks (the engine's drain)
            // on the same Choreographer tick.
            self.main_loop.poll();
        }
        /// Build a rendering instance over a freshly-created wgpu surface
        /// backed by the given Android `Surface`'s `ANativeWindow*`, using the
        /// runtime's shared `wgpu::Instance`. `frame_loop` drives the wake
        /// cadence.
        ///
        /// `configure_instance` receives the [`TurAppBuilder`] BEFORE
        /// `.renderer(…)` is applied — chain
        /// [`TurAppBuilder::instance_data`] (or any other pre-build hook)
        /// on it and return it. The surface-backed renderer is set up by
        /// this function after the closure returns, so the embedder cannot
        /// accidentally override it. Pass `|b| b` for the no-op default.
        ///
        /// Architecture: the engine runs on a worker thread; `MainBackend`
        /// owns the wgpu `VelloRenderer` on the caller thread (main) and
        /// drives it directly — command batches, incremental image uploads,
        /// and resize-on-event.
        #[allow(clippy::too_many_arguments)]
        pub async fn build_with_surface(
            runtime: &AndroidRuntime,
            default_worker_pool: WorkerPoolHandle,
            _tokio: &tokio::runtime::Handle,
            wgpu_instance: &wgpu::Instance,
            window_handle: AndroidWindowHandle,
            logical_width: u32,
            logical_height: u32,
            dpr: f64,
            frame_loop: FrameLoopRef,
            configure_instance: impl for<'a> FnOnce(TurAppBuilder<'a>) -> TurAppBuilder<'a>,
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

            // Apply the embedder's pre-build customization (e.g.
            // `.instance_data(...)`, `.worker_pool(...)`), then attach the
            // surface-backed renderer and build. The engine runs on a
            // worker; `MainBackend` owns the wgpu renderer on main and
            // drives it directly (render batches, image uploads,
            // resize-on-event) — no render_sink callback.
            let app = configure_instance(
                runtime
                    .runtime
                    .app_builder()
                    .worker_pool(default_worker_pool.clone()),
            )
            .renderer(
                Box::new(renderer),
                (logical_width as f64, logical_height as f64),
                dpr,
            )
            .build()?;

            let (vsync, loop_task, vsync_wake_fn) =
                Self::install_frame_loop(&app, frame_loop, &runtime.main_loop);

            Ok(Self {
                app,
                vsync,
                main_loop: runtime.main_loop.clone(),
                loop_task,
                vsync_wake_fn,
            })
        }

        /// Build a headless instance (no surface, no rendering) from the
        /// runtime. Runs JS + capabilities + events only.
        ///
        /// `configure_instance` receives the [`TurAppBuilder`] BEFORE
        /// `.build_headless(…)` is applied — chain
        /// [`TurAppBuilder::instance_data`] on it and return it. Pass
        /// `|b| b` for the no-op default.
        pub fn build_headless(
            runtime: &AndroidRuntime,
            default_worker_pool: WorkerPoolHandle,
            _tokio: &tokio::runtime::Handle,
            frame_loop: FrameLoopRef,
            configure_instance: impl for<'a> FnOnce(TurAppBuilder<'a>) -> TurAppBuilder<'a>,
        ) -> Result<Self, TurAndroidError> {
            let app = configure_instance(
                runtime
                    .runtime
                    .app_builder()
                    .worker_pool(default_worker_pool.clone()),
            )
            .build_headless((0.0, 0.0))?;
            let (vsync, loop_task, vsync_wake_fn) =
                Self::install_frame_loop(&app, frame_loop, &runtime.main_loop);

            Ok(Self {
                app,
                vsync,
                main_loop: runtime.main_loop.clone(),
                loop_task,
                vsync_wake_fn,
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
    use tur_engine::{TurAppBuilder, TurRuntimeBuilder};

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
            _default_worker_pool: tur_engine::WorkerPoolHandle,
            _tokio: &tokio::runtime::Handle,
            _wgpu_instance: &wgpu::Instance,
            _window_handle: AndroidWindowHandle,
            _logical_width: u32,
            _logical_height: u32,
            _dpr: f64,
            _frame_loop: FrameLoopRef,
            _configure_instance: impl for<'a> FnOnce(TurAppBuilder<'a>) -> TurAppBuilder<'a>,
        ) -> Result<Self, TurAndroidError> {
            Err(TurAndroidError::AndroidOnly)
        }

        pub fn build_headless(
            _runtime: &std::rc::Rc<tur_engine::TurRuntime>,
            _default_worker_pool: tur_engine::WorkerPoolHandle,
            _tokio: &tokio::runtime::Handle,
            _frame_loop: FrameLoopRef,
            _configure_instance: impl for<'a> FnOnce(TurAppBuilder<'a>) -> TurAppBuilder<'a>,
        ) -> Result<Self, TurAndroidError> {
            Err(TurAndroidError::AndroidOnly)
        }
    }
}

#[cfg(not(target_os = "android"))]
#[allow(unused_imports)]
pub use imp::{AndroidInstance, AndroidRuntime, TurAndroidError};
