use std::rc::Rc;
use std::sync::Arc;

use boa_engine::Context;
use boa_engine::context::time::Clock;
use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;

use crate::TurApp;
use crate::core::app::TurAppInternal;
use crate::core::app::render;
use crate::core::async_::TurJobExecutor;
use crate::core::capability::{Capabilities, CapabilityDecls};
use crate::core::dev::dev_tool;
use crate::core::edgy::reactive;
use crate::core::fonts::{FontContext, FontLoader};
use crate::core::js_runtime::helpers::FnEntry;
use crate::core::js_runtime::instance_context::InstanceDataCx;
use crate::core::js_runtime::js_value::IntoJs;
use crate::core::js_runtime::module_loader::{bound_native, build_native_module};
use crate::core::js_runtime::{BoaOpaque, TurModuleLoader};
use crate::core::plugin::{AsyncPluginContext, CompileContext, Plugin, PluginContext};
use crate::core::screen::Screen;
use crate::error::TurError;

pub mod backend;
pub use backend::MainBackend;
// `WorkerBackend` is `pub(crate)` — internal to the engine, only
// `MainBackend` (which owns a worker running `WorkerBackend`) is public.
pub(crate) use backend::WorkerBackend;
// `MsgOutcome` is `pub(crate)` — the result of the single shared message
// handler (`MainBackend::apply_msg`), consumed by both `pump` and
// `TurApp::run_loop` (the latter lives in `lib.rs`).
pub(crate) use backend::MsgOutcome;

/// boa's `ContextBuilder::clock<C: Clock + 'static>` is generic over a
/// concrete (`Sized`) `C`, so it won't accept an already-erased
/// `Arc<dyn Clock + Send + Sync>`. `ClockProxy` is a Sized adapter that
/// delegates to the shared `Arc<dyn Clock + Send + Sync>` — giving every
/// instance's boa `Context` and the runtime `Shell` one shared time
/// source. `Send + Sync` so the runtime can be shared across worker
/// threads (Phase 8 threaded mode).
#[derive(Clone)]
pub(crate) struct ClockProxy(pub(crate) Arc<dyn Clock + Send + Sync>);
impl Clock for ClockProxy {
    fn now(&self) -> boa_engine::context::time::JsInstant {
        self.0.now()
    }
    fn system_time_millis(&self) -> i64 {
        self.0.system_time_millis()
    }
}

/// Deferred capability-insert closure (the per-worker replay). Built by a
/// [`CapabilityBuilder`] (which receives the [`AsyncPluginContext`]) and
/// stored on the runtime; each worker spawned from
/// [`app_builder`](TurRuntime::app_builder) replays its
/// closures into its own fresh `Capabilities`. The closure is `Fn` (not
/// `FnOnce`) + `Send + Sync` so it can be stored on the runtime and replayed
/// into every worker. Each call inserts a clone of the captured capability
/// (Capability newtypes wrap `Arc` — cloning is cheap).
type CapabilityInsert = Box<dyn Fn(&Capabilities) + Send + Sync>;

/// Deferred capability-construction closure. Captures the embedder's backend
/// construction (which may need the [`AsyncPluginContext`], e.g. an OS-API
/// backend that hops calls onto the main thread) and produces a
/// [`CapabilityInsert`] replay closure. Runs **once** in `build()` (after the
/// main-thread channel is created) — the resulting backend is shared across
/// every instance (Capability newtypes are `Arc`-backed). `FnOnce` + `Send +
/// Sync` so it can be stored on the builder and consumed in `build()`.
type CapabilityBuilder =
    Box<dyn FnOnce(&AsyncPluginContext) -> Result<CapabilityInsert, TurError> + Send + Sync>;

/// Deferred per-instance data definer closure. Captured on the
/// `TurAppBuilder` (main thread) and replayed once on the worker inside
/// [`build_worker_backend`] — right after `TurInstanceContext` is
/// constructed, before any plugin `register`. The closure receives an
/// [`InstanceDataCx`] exposing **only** [`InstanceDataCx::define`] (the
/// build-time-only initial value for each typed slot).
///
/// Because the closure **runs on the worker**, values built fresh in its
/// body never cross the main↔worker boundary; only values captured by the
/// closure need to be `Send` (hence the `Send + 'static` bound). This is
/// the per-instance counterpart to the per-runtime `CapabilityBuilder` —
/// but simpler: no `AsyncPluginContext` parameter (the definer is pure
/// data, no OS-API hop needed at definition time) and `FnOnce` (one
/// instance per closure).
type InstanceDataDefiner = Box<dyn FnOnce(&mut InstanceDataCx) + Send + 'static>;

/// The shared engine runtime — created **once** and used to spawn any number
/// of isolated [`TurApp`] instances.
///
/// Owns the expensive, instance-independent resources:
/// - the clock (one shared time source),
/// - the font context (system-font discovery + preset fonts, built once;
///   each instance cheaply clones it — `FontContext`/`fontique::Collection`/
///   `System` are `Arc`-backed),
/// - the capability registry (shared Clipboard/Http/FilePicker/Cursor
///   backends),
/// - the registered plugins (their `register` takes `&self`, so the same
///   plugin objects register into every instance's fresh boa `Context`).
///
/// Spawn instances via [`TurRuntime::app_builder`] (rendering or headless).
///
/// ```no_run
///
/// # use tur_engine::*;
/// # use tur_engine::core::fonts::FontLoader;
/// # use tur_engine::core::render::Renderer;
/// # use tur_engine::renderer::noop::NoopRenderer;
/// # fn _doc(loader: std::sync::Arc<dyn tur_engine::core::fonts::FontLoader>) -> Result<(), tur_engine::error::TurError> {
/// let runtime = TurRuntime::builder()
///     .font_loader(loader)
///     .clock(std::sync::Arc::new(boa_engine::context::time::StdClock::new()))
///     .plugin(TurStdPlugin)
///     .build()?;
///
/// // Two isolated instances sharing fonts/clock/capabilities/plugins.
/// // Each owns its renderer (created by the embedder; no render_sink):
/// let app_a = runtime
///     .app_builder()
///     .renderer(Box::new(NoopRenderer::new()), (800.0, 600.0), 2.0)
///     .build()?;
/// let app_b = runtime
///     .app_builder()
///     .renderer(Box::new(NoopRenderer::new()), (400.0, 300.0), 1.0)
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct TurRuntime {
    clock: Arc<dyn Clock + Send + Sync>,
    font_context: FontContext,
    font_loader: Arc<dyn FontLoader>,
    capabilities: Capabilities,
    /// Re-playable capability inserts. Each worker spawned from
    /// [`app_builder`](TurRuntime::app_builder) replays these closures into
    /// its own fresh `Capabilities` (the runtime's `Capabilities` uses
    /// `Rc<RefCell<…>>` — `!Send`, can't cross threads directly; the
    /// closures capture clonable capability newtypes and re-insert on the
    /// worker).
    capability_inserts: Arc<Vec<CapabilityInsert>>,
    /// `Arc` so a threaded factory closure can cheaply clone the plugin
    /// list and re-register every plugin on the worker thread (Phase 8
    /// threaded mode). Each `Box<dyn Plugin>` is `Send + Sync`
    /// (Phase 7 prep), and `register` takes `&self`, so the same plugin
    /// objects work for both inline and threaded instances.
    plugins: Arc<Vec<Box<dyn Plugin>>>,
    /// Main-thread scheduling view. Built once from the driver supplied to
    /// the runtime builder; cloned into each `TurApp` + `MainBackend`.
    main_scheduler: crate::core::scheduler::MainScheduler,
    /// The engine's main-thread hop context. Created internally in `build()`
    /// (the channel + drain are engine-internal — embedders never wire them).
    /// Cloned into each worker's [`PluginContext`] (so plugins reach main via
    /// [`PluginContext::to_async`](crate::core::plugin::PluginContext::to_async))
    /// and handed to capability constructors that need it (via the closure
    /// form of [`TurRuntimeBuilder::capability`]).
    main_cx: AsyncPluginContext,
}

impl TurRuntime {
    pub fn builder() -> TurRuntimeBuilder {
        TurRuntimeBuilder::new()
    }

    /// Read-only access to the shared capability registry.
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Begin building an isolated [`TurApp`] instance. Configure the
    /// surface with [`TurAppBuilder::renderer`], then terminate with
    /// [`TurAppBuilder::build`] (rendering) or
    /// [`TurAppBuilder::build_headless`] (no renderer).
    ///
    /// The engine runs on a worker thread (via [`MainBackend`]); the
    /// renderer lives on the main thread and is owned by `MainBackend`
    /// (passed to `build`). `MainBackend` applies each `Vec<RenderCommand>`
    /// batch directly to the renderer, uploads new image resources
    /// incrementally, and calls `renderer.resize(...)` on viewport-change
    /// events only.
    ///
    /// The instance gets its own boa `Context` (JS realm), element tree,
    /// reactive store, focus manager, event queues, subsystems, screen and
    /// shell — fully isolated from every other instance. Fonts, clock, and
    /// capability backends are shared from this runtime. Plugins are
    /// re-registered into the instance's fresh realm.
    ///
    /// **Capabilities are NOT shared with the worker** — the runtime's
    /// `Capabilities` is `Rc<RefCell<…>>` (`!Send`). The threaded factory
    /// constructs a fresh, empty `Capabilities` on the worker; embedders
    /// that need capabilities (Clipboard/Http/FilePicker/Cursor) should
    /// construct them inside a custom factory via the lower-level
    /// [`build_worker_backend`] helper.
    pub fn app_builder(self: &Rc<Self>) -> TurAppBuilder<'_> {
        TurAppBuilder {
            runtime: self,
            renderer: None,
            viewport: None,
            dpr: None,
            instance_data_definer: None,
        }
    }

    /// Internal: build an instance from already-resolved config. Called by
    /// both [`TurAppBuilder::build`] (rendering) and
    /// [`TurAppBuilder::build_headless`] (the latter passes a
    /// [`NoopRenderer`](crate::renderer::NoopRenderer)).
    fn spawn_instance(
        self: &Rc<Self>,
        renderer: Box<dyn crate::core::render::Renderer>,
        viewport: (f64, f64),
        dpr: f64,
        instance_data_definer: Option<InstanceDataDefiner>,
    ) -> Result<Rc<TurApp>, TurError> {
        let clock = self.clock.clone();
        let font_context = self.font_context.clone();
        let font_loader = self.font_loader.clone();
        let plugins = self.plugins.clone();
        let capability_inserts = self.capability_inserts.clone();
        let main_cx = self.main_cx.clone();
        let backend_factory = move |worker_sched: crate::core::scheduler::WorkerScheduler,
                                    wake_worker: std::sync::Arc<dyn Fn() + Send + Sync>,
                                    main_tx: crate::core::app::MainTx|
              -> WorkerBackend {
            let capabilities = Capabilities::new();
            for insert_fn in capability_inserts.iter() {
                insert_fn(&capabilities);
            }
            build_worker_backend(
                clock,
                font_context,
                font_loader,
                capabilities,
                &plugins,
                viewport,
                worker_sched,
                wake_worker,
                main_tx,
                main_cx.clone(),
                instance_data_definer,
            )
            .expect("threaded backend factory failed")
        };
        let backend = MainBackend::new(self.main_scheduler.clone(), renderer, backend_factory);
        let app = Rc::new(TurApp::new(backend, self.main_scheduler.clone()));
        // Bootstrap the viewport: resize the main-side renderer directly
        // AND seed the worker's screen state + `viewportSize$` atom before
        // frame 1 (the forwarded `PlatformEvent::Resize` does the worker
        // half).
        app.resize(viewport.0 as u32, viewport.1 as u32, dpr);
        Ok(app)
    }
}

/// Builder for an isolated [`TurApp`] instance, started via
/// [`TurRuntime::app_builder`]. Configure the surface with
/// [`Self::renderer`], then terminate with [`Self::build`] (rendering —
/// requires [`Self::renderer`]) or [`Self::build_headless`] (no renderer).
///
/// ```no_run
/// # use tur_engine::*;
/// # use tur_engine::renderer::noop::NoopRenderer;
/// # use std::rc::Rc;
/// # fn _doc(runtime: Rc<TurRuntime>) -> Result<(), tur_engine::error::TurError> {
/// let app = runtime
///     .app_builder()
///     .renderer(Box::new(NoopRenderer::new()), (800.0, 600.0), 2.0)
///     .build()?;
/// # Ok(())
/// # }
/// /// ```
pub struct TurAppBuilder<'rt> {
    runtime: &'rt Rc<TurRuntime>,
    /// Rendering surface: renderer + viewport + dpr grouped together (a
    /// non-headless app supplies all three at once via [`Self::renderer`]).
    /// `None` until [`Self::renderer`] is called; [`Self::build`] requires
    /// `Some`.
    renderer: Option<Box<dyn crate::core::render::Renderer>>,
    viewport: Option<(f64, f64)>,
    dpr: Option<f64>,
    /// Optional build-time definer for per-instance data. See
    /// [`Self::instance_data`]. Replayed once on the worker inside
    /// `build_worker_backend` before any plugin `register`.
    instance_data_definer: Option<InstanceDataDefiner>,
}

impl<'rt> TurAppBuilder<'rt> {
    /// Group the rendering surface — renderer, viewport, dpr — onto this
    /// builder. A non-headless app must supply all three together; the
    /// terminal [`Self::build`] then takes no arguments. `MainBackend`
    /// owns the renderer on main and drives it directly (render batches,
    /// image uploads, resize-on-event) — no `render_sink` callback.
    ///
    /// `viewport` is the initial logical `(width, height)` of the render
    /// target; `dpr` is the device pixel ratio. The engine pushes the
    /// initial `PlatformEvent::Resize` into the worker so `viewportSize$`
    /// + the main-side renderer are seeded before frame 1.
    pub fn renderer(
        mut self,
        renderer: Box<dyn crate::core::render::Renderer>,
        viewport: (f64, f64),
        dpr: f64,
    ) -> Self {
        self.renderer = Some(renderer);
        self.viewport = Some(viewport);
        self.dpr = Some(dpr);
        self
    }

    /// Define build-time per-instance data. The closure receives an
    /// [`InstanceDataCx`] exposing **only** [`InstanceDataCx::define`]
    /// — the initial value for each typed slot. Call `define::<T>(value)`
    /// once per type; defining the same type twice panics (fail-fast).
    ///
    /// The closure **runs on the worker**, right after the instance's
    /// `TurInstanceContext` is constructed and **before** any plugin
    /// `register`. This means:
    /// - Values built fresh inside the closure body never cross the
    ///   main↔worker boundary (only captured values need `Send`).
    /// - Plugins see all defined slots as already-present at `register`
    ///   time, so they can only [`TurInstanceContext::update`] (replace)
    ///   or [`TurInstanceContext::data`] / [`TurInstanceContext::with_data`]
    ///   (read) — they cannot introduce new types.
    ///
    /// Per-instance by design: call this on each `app_builder()` if you
    /// want the same data on every instance.
    ///
    /// ```no_run
    /// # use tur_engine::*;
    /// # use std::rc::Rc;
    /// # fn _doc(runtime: Rc<TurRuntime>) -> Result<(), tur_engine::error::TurError> {
    /// # struct PluginId(String);
    /// # struct ThemeConfig { dark: bool }
    /// let app = runtime
    ///     .app_builder()
    ///     .instance_data(|cx| {
    ///         cx.define::<PluginId>(PluginId("com.example.foo".into()));
    ///         cx.define::<ThemeConfig>(ThemeConfig { dark: true });
    ///     })
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn instance_data<F>(mut self, define: F) -> Self
    where
        F: FnOnce(&mut InstanceDataCx) + Send + 'static,
    {
        self.instance_data_definer = Some(Box::new(define));
        self
    }

    /// Terminal: build the instance. Requires [`Self::renderer`] to have
    /// been called (a non-headless app supplies renderer + viewport + dpr
    /// together). Errors with a clear message otherwise.
    pub fn build(self) -> Result<Rc<TurApp>, TurError> {
        let TurAppBuilder {
            runtime,
            renderer,
            viewport,
            dpr,
            instance_data_definer,
        } = self;
        let renderer = renderer.ok_or_else(|| {
            TurError::Other(
                "TurAppBuilder::build() requires `.renderer(renderer, viewport, dpr)` \
                 to have been called; for a headless build use `.build_headless(viewport)` \
                 instead"
                    .to_string(),
            )
        })?;
        let viewport = viewport.expect("renderer() sets viewport atomically with renderer");
        let dpr = dpr.expect("renderer() sets dpr atomically with renderer");
        runtime.spawn_instance(renderer, viewport, dpr, instance_data_definer)
    }

    /// Terminal: build a headless instance (no renderer, no rendering).
    /// The instance still runs JS, owns a reactive store, accepts platform
    /// events if fed any, and can use capabilities (http, clipboard, etc.).
    /// The engine runs on a worker thread (via [`MainBackend`]) — JS
    /// execution, frame flushes, and every `async` RPC round-trip through
    /// the same main↔worker channel as a rendering instance; the only
    /// difference is the main-side [`Renderer`](crate::core::render::Renderer)
    /// is a [`NoopRenderer`](crate::renderer::NoopRenderer), so paint
    /// batches are discarded.
    ///
    /// `viewport` sets the initial `viewportSize$` (read by JS layout);
    /// pass `(0.0, 0.0)` if layout is irrelevant.
    pub fn build_headless(self, viewport: (f64, f64)) -> Result<Rc<TurApp>, TurError> {
        let TurAppBuilder {
            runtime,
            instance_data_definer,
            ..
        } = self;
        runtime.spawn_instance(
            Box::new(crate::renderer::NoopRenderer::new()),
            viewport,
            1.0,
            instance_data_definer,
        )
    }
}

/// Construct a [`WorkerBackend`] from individual engine pieces. Used by
/// [`TurAppBuilder::build`] (via a factory closure that runs on the worker
/// thread) and by embedders that need to construct the worker backend with
/// a custom pre-populated `Capabilities` (e.g. for
/// Clipboard/Http/FilePicker).
///
/// The function itself is callable from any thread — the caller is
/// responsible for ensuring `clock`, `font_loader`, etc. are constructed
/// on the right thread (e.g. the threaded factory constructs them inside
/// the closure so the `!Send` `Rc`s never cross threads).
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_worker_backend(
    clock: Arc<dyn Clock + Send + Sync>,
    font_context: FontContext,
    font_loader: Arc<dyn FontLoader>,
    capabilities: crate::core::capability::Capabilities,
    plugins: &[Box<dyn Plugin>],
    viewport: (f64, f64),
    worker_sched: crate::core::scheduler::WorkerScheduler,
    wake_worker: std::sync::Arc<dyn Fn() + Send + Sync>,
    main_tx: crate::core::app::MainTx,
    async_cx: AsyncPluginContext,
    instance_data_definer: Option<InstanceDataDefiner>,
) -> Result<WorkerBackend, TurError> {
    let executor = Rc::new(TurJobExecutor::new());
    let module_loader = TurModuleLoader::new();
    let mut boa_context = Context::builder()
        .clock(Rc::new(ClockProxy(clock.clone())))
        .job_executor(executor.clone())
        .module_loader(module_loader.clone())
        .build()
        .expect("failed to build boa context");

    let internal = TurAppInternal::new(
        font_context,
        font_loader,
        executor.clone(),
        clock,
        capabilities,
        worker_sched,
        wake_worker,
        main_tx,
    );

    let opaque = BoaOpaque::new(internal.js_context.clone(), &mut boa_context);
    let ctx_val: boa_engine::JsValue = opaque.object().clone().into();

    let viewport_size_js: boa_engine::JsValue = {
        internal.app_context.borrow_mut().screen.logical_size = viewport;
        let init = Screen::size_js(viewport.0, viewport.1, &mut boa_context);
        let src: reactive::Source<boa_engine::JsValue> =
            internal.js_context.store.bridge().source(init);
        internal.app_context.borrow_mut().screen.set_source(src);
        src.into_js(&mut boa_context)
    };

    let mut core_fns: Vec<FnEntry> = Vec::new();
    core_fns.extend(crate::core::edgy::bridge::fns());
    core_fns.extend(render::fns());
    let core_module = build_native_module(
        &mut boa_context,
        opaque.object().clone().into(),
        &core_fns,
        &[],
        &[],
    );
    module_loader.register("tur:core", core_module);

    let dt_obj = JsObject::with_object_proto(boa_context.intrinsics());
    let et_fn = bound_native(
        &mut boa_context,
        ctx_val.clone(),
        dev_tool::tur_dev_tool_element_tree,
        0,
        "elementTree",
    );
    let ge_fn = bound_native(
        &mut boa_context,
        ctx_val.clone(),
        dev_tool::tur_dev_tool_get_element,
        1,
        "getElement",
    );
    let _ = dt_obj.create_data_property(
        js_string!("elementTree"),
        boa_engine::JsValue::from(et_fn),
        &mut boa_context,
    );
    let _ = dt_obj.create_data_property(
        js_string!("getElement"),
        boa_engine::JsValue::from(ge_fn),
        &mut boa_context,
    );
    let _ =
        boa_context.register_global_property(js_string!("turDevTool"), dt_obj, Attribute::all());

    // Replay the build-time `instance_data` definer (from
    // `TurAppBuilder::instance_data`) — runs on the worker, before any
    // plugin `register`, so plugins see all defined slots as already
    // present (they can `update` / `data` / `with_data` but not define).
    if let Some(definer) = instance_data_definer {
        let mut data_cx = InstanceDataCx::from_map(internal.js_context.instance_data.clone());
        definer(&mut data_cx);
    }

    for plugin in plugins {
        let mut plugin_ctx = PluginContext {
            boa: &mut boa_context,
            loader: module_loader.clone(),
            js_ctx_value: ctx_val.clone(),
            js_ctx: internal.js_context.clone(),
            app: internal.app_context.clone(),
            subsystems: internal.subsystems.clone(),
            event_bus: internal.event_bus.clone(),
            viewport_size: viewport_size_js.clone(),
            async_cx: async_cx.clone(),
        };
        plugin.register(&mut plugin_ctx)?;
    }

    {
        let cursor_backend = internal
            .js_context
            .capability()
            .of::<crate::core::platform::CursorCap>()
            .map(|c| c.backend().clone())
            .unwrap_or_else(|| {
                std::sync::Arc::new(std::sync::Mutex::new(crate::core::platform::NoopCursor))
            });
        internal
            .app_context
            .borrow_mut()
            .shell
            .set_cursor_platform(cursor_backend);
    }

    tracing::info!("WorkerBackend built ({} plugins)", plugins.len());
    Ok(WorkerBackend::new(boa_context, internal, executor))
}

pub struct TurRuntimeBuilder {
    font_loader: Option<Arc<dyn FontLoader>>,
    clock: Option<Arc<dyn Clock + Send + Sync>>,
    plugins: Vec<Box<dyn Plugin>>,
    capability_builders: Vec<CapabilityBuilder>,
    scheduler: Option<Rc<dyn crate::core::scheduler::MainSchedulerDriver>>,
}

impl Default for TurRuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TurRuntimeBuilder {
    pub fn new() -> Self {
        Self {
            font_loader: None,
            clock: None,
            plugins: Vec::new(),
            capability_builders: Vec::new(),
            scheduler: None,
        }
    }

    /// Provide the font loader. The runtime builds one `FontContext` from it
    /// (discovering system fonts + loading presets once) and each instance
    /// clones that context. Required. Stored as `Arc` so it can be shared
    /// across worker threads in Phase 8 threaded mode.
    pub fn font_loader(mut self, font_loader: Arc<dyn FontLoader>) -> Self {
        self.font_loader = Some(font_loader);
        self
    }

    /// Provide the engine clock — the single source of time read by JS
    /// `Date.now()` and timer scheduling, shared across every instance.
    /// Required.
    ///
    /// Production passes a [`StdClock`] (real wall clock); tests pass a
    /// [`FixedClock`] they advance themselves frame-by-frame.
    ///
    /// [`StdClock`]: boa_engine::context::time::StdClock
    /// [`FixedClock`]: boa_engine::context::time::FixedClock
    pub fn clock(mut self, clock: Arc<dyn Clock + Send + Sync>) -> Self {
        self.clock = Some(clock);
        self
    }

    pub fn plugin<P: Plugin + 'static>(mut self, plugin: P) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Like [`plugin`](Self::plugin) but accepts an already-boxed plugin.
    pub fn plugin_boxed(mut self, plugin: Box<dyn Plugin>) -> Self {
        self.plugins.push(plugin);
        self
    }

    /// Register a capability (a plugin-swappable backend) so it's available
    /// to every instance spawned from this runtime. Capabilities are
    /// constructed once in [`build`](Self::build) (after the engine creates
    /// its main-thread channel) and replayed into each worker's fresh
    /// `Capabilities` during
    /// [`app_builder`](TurRuntime::app_builder) →
    /// [`TurAppBuilder::build`](TurAppBuilder::build).
    ///
    /// The closure receives an [`AsyncPluginContext`] clone — the engine's
    /// main-thread hop. Backends that need to run OS-API calls on the main
    /// thread (e.g. macOS `arboard`/`NSPasteboard`) store it and self-hop
    /// via [`AsyncPluginContext::run_on_main`]; backends that don't (wasm,
    /// HTTP via tokio, filepicker via `rfd`) ignore the argument. The
    /// closure is `FnOnce`: the backend is constructed exactly once and
    /// shared (Capability newtypes wrap `Arc`, so per-worker replay is a
    /// cheap clone).
    ///
    /// May fail at `build()` (e.g. no clipboard available on headless CI) —
    /// the `Err` propagates from `build()`.
    ///
    /// Plugins declare hard dependencies via [`Plugin::requires`]; the
    /// runtime validates those before any plugin side effects.
    ///
    /// ```no_run
    /// # use tur_engine::{Clipboard, ClipboardBackend, TurRuntimeBuilder};
    /// # use std::future::Future;
    /// # use std::pin::Pin;
    /// # struct MyBackend;
    /// # impl ClipboardBackend for MyBackend {
    /// #     fn read_text(&self) -> Pin<Box<dyn Future<Output = String>>> { Box::pin(async { String::new() }) }
    /// #     fn write_text(&self, _t: String) -> Pin<Box<dyn Future<Output = ()>>> { Box::pin(async {}) }
    /// # }
    /// TurRuntimeBuilder::new()
    ///     // A backend that needs main-thread access takes the context:
    ///     //   .capability(|cx| Ok(Clipboard::new(my_native_clipboard(cx)?)))
    ///     // A backend that doesn't ignores it:
    ///     .capability(|_| Ok(Clipboard::new(MyBackend)))
    ///     # ;
    /// ```
    pub fn capability<C, F>(mut self, build: F) -> Self
    where
        C: crate::core::capability::Capability + Send + Sync + 'static,
        F: FnOnce(&AsyncPluginContext) -> Result<C, TurError> + Send + Sync + 'static,
    {
        self.capability_builders
            .push(Box::new(move |cx: &AsyncPluginContext| {
                let cap = build(cx)?;
                Ok(Box::new(move |registry: &Capabilities| {
                    registry.insert::<C>(cap.clone());
                }))
            }));
        self
    }

    /// Set the scheduler driver. Required before `build()`. The driver
    /// must implement [`MainSchedulerDriver`]; the runtime wraps it in a
    /// [`MainScheduler`] view. (The per-worker driver is a separate object
    /// constructed inside the driver's own `spawn_worker` impl — the
    /// engine never sees it at build time.)
    ///
    /// [`MainSchedulerDriver`]: crate::core::scheduler::MainSchedulerDriver
    /// [`MainScheduler`]: crate::core::scheduler::MainScheduler
    pub fn scheduler<S>(mut self, driver: Rc<S>) -> Self
    where
        S: crate::core::scheduler::MainSchedulerDriver + 'static,
    {
        self.scheduler = Some(driver);
        self
    }

    pub fn build(self) -> Result<Rc<TurRuntime>, TurError> {
        let font_loader = self
            .font_loader
            .expect("font_loader must be set (use TurRuntimeBuilder::font_loader)");
        let clock = self
            .clock
            .expect("clock must be set (use TurRuntimeBuilder::clock)");
        let scheduler = self
            .scheduler
            .expect("scheduler must be set (use TurRuntimeBuilder::scheduler)");

        // Build the one shared FontContext — system-font discovery + preset
        // loading happen exactly once here. Instances clone it cheaply.
        let mut font_context = FontContext::new();
        font_loader.load_preset_fonts(&mut font_context);

        let main_scheduler = crate::core::scheduler::MainScheduler::new(scheduler);

        // Create the engine-internal main-thread channel + spawn the drain on
        // main. `build()` runs on the main thread, so `spawn_local` is valid
        // here; the drain runs on the next main-executor tick and serves the
        // `AsyncPluginContext` (plugin/bridge hops) for the runtime's life.
        let (tx, drain) = crate::core::scheduler::main_channel();
        main_scheduler.spawn_local(Box::pin(drain.run()));
        let main_cx = AsyncPluginContext::from_sender(tx);

        // Run each capability-construction closure once (receives the
        // `AsyncPluginContext`), producing the per-worker replay closures.
        // A failing backend (e.g. no clipboard on headless CI) propagates
        // `Err` out of `build()`.
        let mut capability_inserts: Vec<CapabilityInsert> = Vec::new();
        for builder in self.capability_builders {
            capability_inserts.push(builder(&main_cx)?);
        }

        let capabilities = Capabilities::new();
        // Replay each insert closure once here (validates that they all
        // run cleanly + populates the runtime's Capabilities for
        // compile-time `requires` validation). The closures are retained
        // on the runtime (`capability_inserts`) and replayed again into
        // every worker's fresh `Capabilities` from `app_builder().build(...)`.
        for insert_fn in &capability_inserts {
            insert_fn(&capabilities);
        }

        // Validate every plugin's `requires` declaration against the registry
        // BEFORE any plugin's `compile` runs. A missing cap fails fast with a
        // clear message naming the missing type and the fix.
        {
            let mut decls = CapabilityDecls::new();
            for plugin in &self.plugins {
                plugin.requires(&mut decls);
            }
            for (cap_id, cap_name) in decls.iter() {
                if !capabilities.contains_id(cap_id) {
                    return Err(TurError::Other(format!(
                        "plugin requires capability `{cap_name}` which is not registered; \
                         add `.capability(|_| Ok({cap_name}::new(...)))` to the runtime builder"
                    )));
                }
            }
        }

        // One-time compile pass: validate module sources, build descriptors.
        {
            let mut compile_cx = CompileContext {
                capabilities: &capabilities,
                font_context: &font_context,
            };
            for plugin in &self.plugins {
                plugin.compile(&mut compile_cx)?;
            }
        }

        tracing::info!("TurRuntime initialized ({} plugins)", self.plugins.len());

        Ok(Rc::new(TurRuntime {
            clock,
            font_context,
            font_loader,
            capabilities,
            capability_inserts: Arc::new(capability_inserts),
            plugins: Arc::new(self.plugins),
            main_scheduler,
            main_cx,
        }))
    }
}
