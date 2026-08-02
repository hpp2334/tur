use std::rc::Rc;

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
pub use backend::{AnySend, BoaClosure, ElementClosure, InlineBackend, TurAppBackend};

/// boa's `ContextBuilder::clock<C: Clock + 'static>` is generic over a
/// concrete (`Sized`) `C`, so it won't accept an already-erased
/// `Rc<dyn Clock>`. `ClockProxy` is a Sized adapter that delegates to the
/// shared `Rc<dyn Clock>` — giving every instance's boa `Context` and the
/// runtime `Shell` one shared time source.
#[derive(Clone)]
struct ClockProxy(Rc<dyn Clock>);
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
/// # use std::rc::Rc;
/// # use tur_engine::*;
/// # use tur_engine::core::fonts::FontLoader;
/// # use tur_engine::core::render::Renderer;
/// # fn _doc(loader: Rc<dyn FontLoader>, renderer_a: Box<dyn Renderer>, renderer_b: Box<dyn Renderer>) -> Result<(), tur_engine::error::TurError> {
/// let runtime = TurRuntime::builder()
///     .font_loader(loader)
///     .clock(Rc::new(boa_engine::context::time::StdClock::new()))
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
    clock: Rc<dyn Clock>,
    font_context: FontContext,
    font_loader: Rc<dyn FontLoader>,
    capabilities: Capabilities,
    plugins: Vec<Box<dyn Plugin>>,
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

    fn create_instance(
        self: &Rc<Self>,
        renderer: Option<Box<dyn Renderer>>,
        viewport: (f64, f64),
    ) -> Result<Rc<TurApp>, TurError> {
        let renderer = renderer.unwrap_or_else(|| Box::new(crate::renderer::NoopRenderer::new()));

        let executor = Rc::new(TurJobExecutor::new());
        let module_loader = TurModuleLoader::new();
        let mut boa_context = Context::builder()
            .clock(Rc::new(ClockProxy(self.clock.clone())))
            .job_executor(executor.clone())
            .module_loader(module_loader.clone())
            .build()
            .expect("failed to build boa context");

        // Each instance clones the runtime's pre-built FontContext (cheap —
        // Arc-backed) and shares the runtime's font loader (for runtime
        // register_font calls).
        let internal = TurAppInternal::new(
            renderer,
            self.font_context.clone(),
            self.font_loader.clone(),
            executor.clone(),
            self.clock.clone(),
            self.capabilities.clone(),
        );

        let opaque = BoaOpaque::new(internal.js_context.clone(), &mut boa_context);
        let ctx_val: boa_engine::JsValue = opaque.object().clone().into();

        // Engine-owned `viewportSize$` reactive source. Seeded with the
        // instance's initial viewport; updated on each `PlatformEvent::Resize`
        // by `core::screen::ResizeSubsystem`. We also set `screen.logical_size`
        // directly so headless instances (which never receive a Resize event)
        // lay out against the right dimensions.
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

        // Per-instance turDevTool global (each instance has its own boa
        // Context, so its own dev-tool object).
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
        let _ = boa_context.register_global_property(
            js_string!("turDevTool"),
            dt_obj,
            Attribute::all(),
        );

        // Re-register every plugin into this instance's fresh boa Context.
        // `register` takes `&self`, so the same plugin objects are reused.
        for plugin in &self.plugins {
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

        // Install the cursor backend on the Shell from the shared capability
        // registry. Falls back to `NoopCursor` when no cursor capability is
        // present.
        {
            let cursor_backend = internal
                .js_context
                .capability()
                .of::<crate::core::platform::CursorCap>()
                .map(|c| c.backend().clone())
                .unwrap_or_else(|| {
                    Rc::new(std::cell::RefCell::new(crate::core::platform::NoopCursor))
                });
            internal
                .app_context
                .borrow_mut()
                .shell
                .set_cursor_platform(cursor_backend);
        }

        tracing::info!("TurApp instance created ({} plugins)", self.plugins.len());

        let backend = InlineBackend::new(boa_context, internal, executor);
        Ok(Rc::new(TurApp::new(Box::new(backend))))
    }
}

pub struct TurRuntimeBuilder {
    font_loader: Option<Rc<dyn FontLoader>>,
    clock: Option<Rc<dyn Clock>>,
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
    /// clones that context. Required.
    pub fn font_loader(mut self, font_loader: Rc<dyn FontLoader>) -> Self {
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
    pub fn clock(mut self, clock: Rc<dyn Clock>) -> Self {
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
            plugins: self.plugins,
        }))
    }
}
