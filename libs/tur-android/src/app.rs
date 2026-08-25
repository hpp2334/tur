//! tur Android engine wrapper: builds a shared [`TurRuntime`] once (fonts,
//! clock, capabilities, wgpu instance), then spawns isolated [`TurApp`]
//! instances — each attached to an Android `Surface` (rendering) or headless
//! (no rendering). Exposes the operations the JNI layer drives.
//!
//! All of it lives on the **tur-host thread** (`crate::host_thread`): the
//! JNI layer marshals every op there, so the Android main thread only ever
//! posts work (see `ops` in `lib.rs`). On non-Android targets the crate
//! compiles as a stub so the workspace builds on desktop; this module is
//! then empty.

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
    use tur_engine::{TurApp, TurAppBuilder, TurAppLooper, TurRuntime, TurRuntimeBuilder};
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

    use crate::ModuleSourceRegistry;
    use crate::scheduler::{AndroidHostLoop, AndroidVsyncSource, FrameLoopRef};
    use crate::surface::AndroidWindowHandle;
    use tur_native::NativeFontLoader;

    /// Push the engine's text-input state (editable focused flag) to
    /// Kotlin's `FrameLoop.onTextInputChanged(boolean)` via JNI. Called
    /// from the engine's [`AndroidShell`] (installed at
    /// `app_builder().shell(...)`) on the **tur-host thread** (frames run
    /// there), which `attach_current_thread` attaches on first use. Kotlin
    /// retains the value and reconciles the soft keyboard from it — both
    /// the per-frame sync and a posted reconcile on state change (see
    /// `FrameLoop.onTextInputChanged`).
    fn push_text_input_to_kotlin(frame_loop: &FrameLoopRef, is_editable: bool) {
        let Some(vm) = crate::java_vm() else {
            return;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return;
        };
        let loop_obj = unsafe { JObject::from_raw(frame_loop.kotlin_loop.as_raw()) };
        if let Err(e) = env.call_method(
            &loop_obj,
            "onTextInputChanged",
            "(Z)V",
            &[JValue::Bool(is_editable as jni::sys::jboolean)],
        ) {
            // A mismatch here would silently kill the soft-keyboard sync —
            // log loudly instead of swallowing it.
            tracing::error!("FrameLoop.onTextInputChanged JNI call failed: {e}");
        }
    }

    /// Android's [`tur_engine::Shell`]: forwards text-input (IME session)
    /// state to Kotlin so the per-frame IME sync can raise/lower the soft
    /// keyboard, and carries the instance's frame clock (a
    /// Choreographer-bound [`AndroidVsyncSource`], handed to the engine at
    /// construction). Cursor output is a no-op (touch devices have no
    /// pointer cursor).
    struct AndroidShell {
        frame_loop: FrameLoopRef,
        vsync: Option<Rc<AndroidVsyncSource>>,
    }

    impl tur_engine::Shell for AndroidShell {
        fn set_cursor(&mut self, _cursor: tur_engine::core::shell::Cursor) {}

        fn request_text_input(&mut self, state: tur_engine::core::shell::TextInputState) {
            push_text_input_to_kotlin(&self.frame_loop, state.is_editable);
        }

        fn take_vsync(&mut self) -> Option<Rc<dyn VsyncSource>> {
            self.vsync.take().map(|v| v as Rc<dyn VsyncSource>)
        }
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

    /// The shared runtime — created once per app process, on the **tur-host
    /// thread**. Owns the [`TurRuntime`] (fonts, clock, capabilities,
    /// plugins), a shared `wgpu::Instance` that every rendering instance
    /// creates its `Surface` from, and the tokio runtime whose handle backs
    /// the lane timers, `sleep` timers. The JNI layer's `jlong` runtime
    /// handle points at a
    /// [`RuntimeRoute`](crate::host_thread::RuntimeRoute) — this struct is
    /// reachable only from host-thread ops.
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
        pub host_loop: Rc<AndroidHostLoop>,
        /// Shared registry of `Arc<str>` module sources (the handle-based
        /// module-loading path). Sources registered here — from Kotlin via
        /// `registerModuleSource` or from Rust embedder code — load into any
        /// instance of this runtime by handle, never crossing JNI as a
        /// string. Dropped wholesale with the runtime.
        pub module_sources: ModuleSourceRegistry,
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
        /// installed (native font loader, wall-clock `StdClock`,
        /// `AndroidClipboard`, `NativeHttp`, the base scheduler driver), so the
        /// callback only needs to chain `.plugin(…)` calls and return the
        /// builder.
        ///
        /// Runs on the **tur-host thread** (`ops::create_runtime` marshals
        /// the build over) — the runtime's `Rc` state (host loop, lane
        /// registry) is `!Send` and must never cross a thread. `module_sources`
        /// is the caller-allocated shared registry (the JNI route holds the
        /// other Arc half, so module sources register from any thread).
        pub fn build(
            context: GlobalRef,
            module_sources: ModuleSourceRegistry,
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
            // timers); the frame clock is per-instance — each instance's
            // shell carries a Choreographer-bound source, handed over at
            // `app_builder()...build()`; main-loop tasks polled from each
            // instance's `pump_loop`.
            let worker_spawner = crate::scheduler::worker_spawner(tokio.handle().clone());
            let host_loop = AndroidHostLoop::new();

            // Default (effectively uncapped) worker pool — one dedicated
            // lane thread per instance unless the embedder registers more.
            let default_worker_pool = WorkerPoolHandle::new("default", usize::MAX);

            let mut builder = TurRuntime::builder()
                .worker_spawner(worker_spawner)
                .host_loop(host_loop.clone())
                .font_loader(std::sync::Arc::new(NativeFontLoader::new()))
                .clock(std::sync::Arc::new(StdClock::new()))
                .worker_pool(default_worker_pool.clone())
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
                host_loop,
                module_sources,
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
    /// [`AndroidInstance::build_headless`]. Lives in the tur-host thread's
    /// instance map, addressed by the id Kotlin's `jlong` instance handle
    /// routes to (see
    /// [`InstanceRoute`](crate::host_thread::InstanceRoute)).
    pub struct AndroidInstance {
        pub app: Rc<TurApp>,
        /// Per-instance vsync source bound to this instance's Kotlin
        /// `FrameLoop`. JNI `pump` fires it before polling the loop.
        pub vsync: Rc<AndroidVsyncSource>,
        /// The runtime's shared module-source registry (cloned handle — same
        /// entries). `loadModule` resolves its `jlong` source handle here.
        pub module_sources: ModuleSourceRegistry,
        /// The runtime's main-thread task spawner, polled each `pump`.
        host_loop: Rc<AndroidHostLoop>,
        /// The autonomous frame loop future (`TurAppLooper::run`). The
        /// boa realm lives on the worker, but the loop itself is `!Send`
        /// host-thread state (it owns the worker→host message stream), so
        /// it lives on the **tur-host thread**: the Choreographer callback
        /// (still on the Android main thread) posts a pump op that polls it
        /// once per display tick.
        loop_task: std::cell::RefCell<Option<Pin<Box<dyn Future<Output = ()>>>>>,
        /// `Send + Sync` closure that requests a loop poll by posting a
        /// poll-only pump op **directly onto the tur-host thread's queue**.
        /// Used as the waker for `pump_loop`: when the worker sends to
        /// `main_rx` (from its own thread), the channel waker fires this
        /// closure → host-thread wake → `pump_loop` runs → processes the
        /// message. No Android-main-thread Handler hop is involved.
        vsync_wake_fn: std::sync::Arc<dyn Fn() + Send + Sync>,
    }

    impl AndroidInstance {
        /// Stash the autonomous loop future for poll-per-`pump` driving,
        /// arm the first vsync (so the bootstrap `FrameOutcome` from
        /// `app_builder()...build()`'s initial resize kicks the loop off),
        /// and register the message wake fn with the runtime's main loop
        /// so pending main-thread tasks (the engine's drain) get a prompt
        /// pump.
        ///
        /// The instance's frame clock was already handed to the engine at
        /// construction: `AndroidShell` carries the Choreographer-bound
        /// [`AndroidVsyncSource`] (built by the caller, cloned here for
        /// JNI `fire_vsync` + the wake fn) — see [`AndroidShell`].
        fn install_frame_loop(
            _app: &Rc<TurApp>,
            looper: TurAppLooper,
            vsync: Rc<AndroidVsyncSource>,
            host_loop: &Rc<AndroidHostLoop>,
        ) -> (
            Rc<AndroidVsyncSource>,
            std::cell::RefCell<Option<Pin<Box<dyn Future<Output = ()>>>>>,
            std::sync::Arc<dyn Fn() + Send + Sync>,
        ) {
            // Bootstrap: arm the first Choreographer callback. Subsequent
            // frames re-arm via the engine's `request_frame` on a
            // `FrameOutcome { schedule: Vsync }`.
            vsync.request_frame();
            let vsync_wake_fn = vsync.make_vsync_wake_fn();
            // Pending main-loop tasks (the engine's drain) request a pump on
            // this instance → the next `pump_loop` polls them.
            host_loop.add_wake_fn(vsync_wake_fn.clone());
            let loop_task = std::cell::RefCell::new(Some(
                Box::pin(looper.run()) as Pin<Box<dyn Future<Output = ()>>>
            ));
            (vsync, loop_task, vsync_wake_fn)
        }

        /// Poll the autonomous loop exactly once. Called from the tur-host
        /// thread's pump op — the vsync variant (Choreographer-fired: fire
        /// vsync + poll) and the message-pump variant (poll only, posted by
        /// the wake fn). Each poll handles at most one vsync/main-msg event,
        /// so the loop is pulled forward by whichever cadence is active —
        /// display frames while animating, one host-thread wake per message
        /// batch while idle.
        ///
        /// Uses a **real waker** (not `noop_waker`) backed by
        /// [`vsync_wake_fn`](Self::vsync_wake_fn): when the worker sends to
        /// `main_rx`, the channel waker fires the closure → a poll-only
        /// pump op lands on the tur-host queue → this method runs again →
        /// processes the message. Without this, worker messages (render
        /// batches, frame outcomes) would sit unconsumed between input
        /// events.
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
            self.host_loop.poll();
        }
        /// Build a rendering instance over a freshly-created wgpu surface
        /// backed by the given Android `Surface`'s `ANativeWindow*`, using the
        /// runtime's shared `wgpu::Instance`. `frame_loop` drives the wake
        /// cadence.
        ///
        /// Runs on the **tur-host thread** (marshalled there by
        /// `ops::create_instance`): the wgpu adapter/device request and the
        /// worker-lane handshake block that thread while the Android main
        /// thread stays free. `host` + `instance_id` wire the cross-thread
        /// wake path (see [`AndroidVsyncSource::new`]).
        ///
        /// `configure_instance` receives the [`TurAppBuilder`] BEFORE
        /// `.renderer(…)` is applied — chain
        /// [`TurAppBuilder::instance_data`] (or any other pre-build hook)
        /// on it and return it. The surface-backed renderer is set up by
        /// this function after the closure returns, so the embedder cannot
        /// accidentally override it. Pass `|b| b` for the no-op default.
        ///
        /// Architecture: the engine runs on a worker thread; `HostBackend`
        /// owns the wgpu `VelloRenderer` on the tur-host thread and drives
        /// it directly — command batches, incremental image uploads, and
        /// resize-on-event. The Android main thread never touches it.
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
            host: crate::host_thread::HostHandle,
            instance_id: u64,
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
            // worker; `HostBackend` owns the wgpu renderer on main and
            // drives it directly (render batches, image uploads,
            // resize-on-event).
            // The shell carries the Choreographer-bound frame clock; the
            // embedder keeps a clone for the host-thread `fire_vsync` + the
            // wake fn. `request_pump` posts a poll-only pump op straight
            // onto the tur-host queue — the direct, main-thread-free wake
            // path (worker messages / host-loop tasks land on the host
            // thread, not the Android main looper).
            let request_pump: std::sync::Arc<dyn Fn() + Send + Sync> =
                std::sync::Arc::new(move || {
                    let host = host.clone();
                    host.post(move |state| {
                        if let Some(instance) = state.instance(instance_id) {
                            instance.pump_loop();
                        }
                    });
                });
            let vsync = AndroidVsyncSource::new(frame_loop.clone(), request_pump);
            let (app, looper) = configure_instance(
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
            .shell(Box::new(AndroidShell {
                frame_loop: frame_loop.clone(),
                vsync: Some(vsync.clone()),
            }))
            .build()?;

            let (vsync, loop_task, vsync_wake_fn) =
                Self::install_frame_loop(&app, looper, vsync, &runtime.host_loop);

            Ok(Self {
                app,
                vsync,
                module_sources: runtime.module_sources.clone(),
                host_loop: runtime.host_loop.clone(),
                loop_task,
                vsync_wake_fn,
            })
        }

        /// Build a headless instance (no surface, no rendering) from the
        /// runtime. Runs JS + capabilities + events only. Like
        /// [`build_with_surface`](Self::build_with_surface), it runs on the
        /// tur-host thread; `host` + `instance_id` wire the wake path.
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
            host: crate::host_thread::HostHandle,
            instance_id: u64,
            configure_instance: impl for<'a> FnOnce(TurAppBuilder<'a>) -> TurAppBuilder<'a>,
        ) -> Result<Self, TurAndroidError> {
            // Headless instances still need a frame clock (the loop races
            // its ticks against worker messages) — same Choreographer
            // binding + direct tur-host wake as the rendering path.
            let request_pump: std::sync::Arc<dyn Fn() + Send + Sync> = {
                let host = host.clone();
                std::sync::Arc::new(move || {
                    let host = host.clone();
                    host.post(move |state| {
                        if let Some(instance) = state.instance(instance_id) {
                            instance.pump_loop();
                        }
                    });
                })
            };
            let vsync = AndroidVsyncSource::new(frame_loop.clone(), request_pump);
            let (app, looper) = configure_instance(
                runtime
                    .runtime
                    .app_builder()
                    .worker_pool(default_worker_pool.clone()),
            )
            .shell(Box::new(AndroidShell {
                frame_loop,
                vsync: Some(vsync.clone()),
            }))
            .build_headless((0.0, 0.0))?;
            let (vsync, loop_task, vsync_wake_fn) =
                Self::install_frame_loop(&app, looper, vsync, &runtime.host_loop);

            Ok(Self {
                app,
                vsync,
                module_sources: runtime.module_sources.clone(),
                host_loop: runtime.host_loop.clone(),
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
            _module_sources: crate::ModuleSourceRegistry,
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
            _host: crate::host_thread::HostHandle,
            _instance_id: u64,
            _configure_instance: impl for<'a> FnOnce(TurAppBuilder<'a>) -> TurAppBuilder<'a>,
        ) -> Result<Self, TurAndroidError> {
            Err(TurAndroidError::AndroidOnly)
        }

        pub fn build_headless(
            _runtime: &std::rc::Rc<tur_engine::TurRuntime>,
            _default_worker_pool: tur_engine::WorkerPoolHandle,
            _tokio: &tokio::runtime::Handle,
            _frame_loop: FrameLoopRef,
            _host: crate::host_thread::HostHandle,
            _instance_id: u64,
            _configure_instance: impl for<'a> FnOnce(TurAppBuilder<'a>) -> TurAppBuilder<'a>,
        ) -> Result<Self, TurAndroidError> {
            Err(TurAndroidError::AndroidOnly)
        }
    }
}

#[cfg(not(target_os = "android"))]
#[allow(unused_imports)]
pub use imp::{AndroidInstance, AndroidRuntime, TurAndroidError};
