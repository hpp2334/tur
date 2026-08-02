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
use crate::core::capability::{Capabilities, Capability, CapabilityDecls};
use crate::core::dev::dev_tool;
use crate::core::edgy::reactive;
use crate::core::fonts::{FontContext, FontLoader};
use crate::core::js_runtime::helpers::FnEntry;
use crate::core::js_runtime::js_value::IntoJs;
use crate::core::js_runtime::module_loader::{bound_native, build_native_module};
use crate::core::js_runtime::{BoaOpaque, TurModuleLoader};
use crate::core::plugin::{CompileContext, Plugin, PluginContext};
use crate::core::render::Renderer;
use crate::core::screen::Screen;
use crate::error::TurError;

pub mod backend;
pub use backend::{
    AnySend, BoaClosure, ElementClosure, InlineBackend, ThreadedBackend, TurAppBackend,
};

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

/// Deferred capability-insert closure. Captures the typed capability value
/// and the static type parameter, so the actual `Capabilities::insert::<C>`
/// call (which requires a static `C`) happens once inside `build()` rather
/// than at registration time. We store closures instead of
/// `Vec<(TypeId, Box<dyn Any>)>` because `Box<dyn Any>` can't be cloned —
/// the closure pattern sidesteps that limitation.
type CapabilityInsert = Box<dyn FnOnce(&Capabilities)>;

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
/// Spawn instances via [`TurRuntime::create_app`] (rendering, attached to a
/// surface) or [`TurRuntime::create_headless_app`] (no rendering — JS +
/// capabilities + events only).
///
/// ```no_run
///
/// # use tur_engine::*;
/// # use tur_engine::core::fonts::FontLoader;
/// # use tur_engine::core::render::Renderer;
/// # fn _doc(loader: std::sync::Arc<dyn tur_engine::core::fonts::FontLoader>, renderer_a: Box<dyn Renderer>, renderer_b: Box<dyn Renderer>) -> Result<(), tur_engine::error::TurError> {
/// let runtime = TurRuntime::builder()
///     .font_loader(loader)
///     .clock(std::sync::Arc::new(boa_engine::context::time::StdClock::new()))
///     .plugin(TurStdPlugin)
///     .build()?;
///
/// // Two isolated instances sharing fonts/clock/capabilities/plugins:
/// let app_a = runtime.create_app(renderer_a, (800.0, 600.0), 2.0)?;
/// let app_b = runtime.create_app(renderer_b, (400.0, 300.0), 1.0)?;
///
/// // A headless instance (no surface, no rendering):
/// let headless = runtime.create_headless_app((0.0, 0.0))?;
/// # Ok(())
/// # }
/// ```
pub struct TurRuntime {
    clock: Arc<dyn Clock + Send + Sync>,
    font_context: FontContext,
    font_loader: Arc<dyn FontLoader>,
    capabilities: Capabilities,
    /// `Arc` so a threaded factory closure can cheaply clone the plugin
    /// list and re-register every plugin on the worker thread (Phase 8
    /// `create_app_threaded`). Each `Box<dyn Plugin>` is `Send + Sync`
    /// (Phase 7 prep), and `register` takes `&self`, so the same plugin
    /// objects work for both inline and threaded instances.
    plugins: Arc<Vec<Box<dyn Plugin>>>,
}

impl TurRuntime {
    pub fn builder() -> TurRuntimeBuilder {
        TurRuntimeBuilder::new()
    }

    /// Read-only access to the shared capability registry.
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Create an isolated [`TurApp`] instance attached to a render target
    /// (a canvas/surface — supplied as a `Box<dyn Renderer>` by the embedder).
    ///
    /// The instance gets its own boa `Context` (JS realm), element tree,
    /// reactive store, focus manager, event queues, subsystems, screen and
    /// shell — fully isolated from every other instance. Fonts, clock, and
    /// capability backends are shared from this runtime. Plugins are
    /// re-registered into the instance's fresh realm.
    ///
    /// `viewport` is the initial logical `(width, height)` of the render
    /// target; `dpr` is the device pixel ratio (passed to
    /// [`Renderer::resize`]).
    pub fn create_app(
        self: &Rc<Self>,
        renderer: Box<dyn Renderer>,
        viewport: (f64, f64),
        dpr: f64,
    ) -> Result<Rc<TurApp>, TurError> {
        let app = self.create_instance(Some(renderer), viewport)?;
        // Push the initial resize so the renderer configures its surface and
        // the `viewportSize$` atom reflects the given size before frame 1.
        app.push_platform_event(crate::core::platform::PlatformEvent::Resize {
            logical_width: viewport.0 as u32,
            logical_height: viewport.1 as u32,
            dpr,
        });
        Ok(app)
    }

    /// Create an isolated headless [`TurApp`] instance — no render target, no
    /// rendering. The instance still runs JS, owns a reactive store, accepts
    /// platform events if fed any, and can use capabilities (http, clipboard,
    /// etc.). Internally backed by a [`NoopRenderer`](crate::renderer).
    ///
    /// `viewport` sets the initial `viewportSize$` (read by JS layout); pass
    /// `(0.0, 0.0)` if layout is irrelevant.
    pub fn create_headless_app(
        self: &Rc<Self>,
        viewport: (f64, f64),
    ) -> Result<Rc<TurApp>, TurError> {
        self.create_instance(None, viewport)
    }

    /// Create an isolated [`TurApp`] instance backed by a **worker thread**.
    /// The engine state (boa `Context`, element tree, reactive store,
    /// subsystems) lives on the worker; the renderer is supplied via
    /// `renderer_factory` (which runs ON THE WORKER THREAD).
    ///
    /// All `TurApp` methods dispatch via mpsc channels: synchronous from
    /// the caller's perspective, blocking on the worker's reply.
    ///
    /// **Limitation (Phase 8.1):** capabilities are NOT shared with the
    /// worker today — the runtime's `Capabilities` is `Rc<RefCell<…>>`
    /// (`!Send`). The threaded factory constructs a fresh, empty
    /// `Capabilities` on the worker; embedders that need capabilities
    /// (Clipboard/Http/FilePicker/Cursor) should construct them inside
    /// the `renderer_factory` closure via the lower-level
    /// [`build_inline_backend`] helper. Phase 8.2 will migrate
    /// `Capabilities` to `Arc<Mutex<…>>` and share.
    ///
    /// **What works cross-thread:** `load_module`/`load_js`/`eval_module`,
    /// `pump`, `push_platform_event`/`push_app_event`/`request_paint`,
    /// `focused_*`, `query_element`, `dev_tool_*`, `render_to_pixels`.
    ///
    /// **What panics:** `event_bus` (deferred), `set_cursor_backend`
    /// (deferred), `with_boa_context`/`with_element` escape hatches
    /// (inline-only by design). See [`ThreadedBackend`] docs.
    ///
    /// Cross-target: uses `std::thread` on native, `wasm_thread` (Web
    /// Workers + `SharedArrayBuffer`) on wasm32 — see
    /// [`ThreadedBackend::new`]. Wasm builds require `--profile wasm-dev`.
    pub fn create_app_threaded(
        self: &Rc<Self>,
        renderer_factory: impl FnOnce() -> Box<dyn Renderer> + Send + 'static,
        viewport: (f64, f64),
        dpr: f64,
    ) -> Result<Rc<TurApp>, TurError> {
        let clock = self.clock.clone();
        let font_context = self.font_context.clone();
        let font_loader = self.font_loader.clone();
        let plugins = self.plugins.clone();
        let backend_factory = move || {
            let renderer = renderer_factory();
            // Fresh, empty capabilities — Phase 8.1 limitation. Embedders
            // needing capabilities should call `build_inline_backend`
            // directly with their own pre-populated `Capabilities`.
            let capabilities = Capabilities::new();
            build_inline_backend(
                clock,
                font_context,
                font_loader,
                capabilities,
                &plugins,
                renderer,
                viewport,
            )
            .expect("threaded backend factory failed")
        };
        let backend = ThreadedBackend::new(backend_factory);
        let app = Rc::new(TurApp::new(Box::new(backend)));
        app.push_platform_event(crate::core::platform::PlatformEvent::Resize {
            logical_width: viewport.0 as u32,
            logical_height: viewport.1 as u32,
            dpr,
        });
        Ok(app)
    }

    fn create_instance(
        self: &Rc<Self>,
        renderer: Option<Box<dyn Renderer>>,
        viewport: (f64, f64),
    ) -> Result<Rc<TurApp>, TurError> {
        let renderer = renderer.unwrap_or_else(|| Box::new(crate::renderer::NoopRenderer::new()));
        let backend = build_inline_backend(
            self.clock.clone(),
            self.font_context.clone(),
            self.font_loader.clone(),
            self.capabilities.clone(),
            &self.plugins,
            renderer,
            viewport,
        )?;
        Ok(Rc::new(TurApp::new(Box::new(backend))))
    }
}

/// Construct an [`InlineBackend`] from individual engine pieces. Both
/// [`TurRuntime::create_instance`] (inline) and the threaded factory
/// closure (Phase 7's [`crate::core::runtime::ThreadedBackend`]) call
/// this. The function itself is callable from any thread — the caller
/// is responsible for ensuring `clock`, `font_loader`, etc. are
/// constructed on the right thread (e.g. the threaded factory constructs
/// them inside the closure so the `!Send` `Rc`s never cross threads).
#[allow(clippy::too_many_arguments)]
pub fn build_inline_backend(
    clock: Arc<dyn Clock + Send + Sync>,
    font_context: FontContext,
    font_loader: Arc<dyn FontLoader>,
    capabilities: crate::core::capability::Capabilities,
    plugins: &[Box<dyn Plugin>],
    renderer: Box<dyn Renderer>,
    viewport: (f64, f64),
) -> Result<InlineBackend, TurError> {
    let executor = Rc::new(TurJobExecutor::new());
    let module_loader = TurModuleLoader::new();
    let mut boa_context = Context::builder()
        .clock(Rc::new(ClockProxy(clock.clone())))
        .job_executor(executor.clone())
        .module_loader(module_loader.clone())
        .build()
        .expect("failed to build boa context");

    let internal = TurAppInternal::new(
        renderer,
        font_context,
        font_loader,
        executor.clone(),
        clock,
        capabilities,
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
        };
        plugin.register(&mut plugin_ctx)?;
    }

    {
        let cursor_backend = internal
            .js_context
            .capability()
            .of::<crate::core::platform::CursorCap>()
            .map(|c| c.backend().clone())
            .unwrap_or_else(|| Rc::new(std::cell::RefCell::new(crate::core::platform::NoopCursor)));
        internal
            .app_context
            .borrow_mut()
            .shell
            .set_cursor_platform(cursor_backend);
    }

    tracing::info!("InlineBackend built ({} plugins)", plugins.len());
    Ok(InlineBackend::new(boa_context, internal, executor))
}

pub struct TurRuntimeBuilder {
    font_loader: Option<Arc<dyn FontLoader>>,
    clock: Option<Arc<dyn Clock + Send + Sync>>,
    plugins: Vec<Box<dyn Plugin>>,
    capabilities: Vec<CapabilityInsert>,
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
            capabilities: Vec::new(),
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
    /// to every instance spawned from this runtime. Capabilities are inserted
    /// into the registry before any plugin's `compile`/`register` runs.
    ///
    /// Plugins declare hard dependencies via [`Plugin::requires`]; the runtime
    /// validates those before any plugin side effects.
    pub fn capability<C: Capability>(mut self, cap: C) -> Self {
        self.capabilities
            .push(Box::new(move |registry: &Capabilities| {
                registry.insert::<C>(cap);
            }));
        self
    }

    pub fn build(self) -> Result<Rc<TurRuntime>, TurError> {
        let font_loader = self
            .font_loader
            .expect("font_loader must be set (use TurRuntimeBuilder::font_loader)");
        let clock = self
            .clock
            .expect("clock must be set (use TurRuntimeBuilder::clock)");

        // Build the one shared FontContext — system-font discovery + preset
        // loading happen exactly once here. Instances clone it cheaply.
        let mut font_context = FontContext::new();
        font_loader.load_preset_fonts(&mut font_context);

        let capabilities = Capabilities::new();
        for insert_fn in self.capabilities {
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
                         add `.capability({cap_name}::new(...))` to the runtime builder"
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
            plugins: Arc::new(self.plugins),
        }))
    }
}
