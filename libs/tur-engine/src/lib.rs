pub mod core;
pub mod elements;
pub mod renderer;

pub mod error;

// Re-export engine-internal cursor-capability types at the crate root so
// embedders can write `tur_engine::CursorBackend`, `tur_engine::CursorCap`,
// `tur_engine::NoopCursor` without reaching into `core::platform`. The std
// plugin itself (`TurStdPlugin`) lives in the separate `tur-std` crate.
pub use crate::core::platform::{CursorBackend, CursorCap, NoopCursor};

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use boa_engine::context::time::Clock;
use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::Context;
use boa_engine::Source;

use error::TurError;

use core::app::{FrameOutcome, TurAppInternal};

use core::bridge::helpers::FnEntry;
use core::bridge::module_loader::{build_native_module, bound_native};
use core::bridge::{console, dev_tool, reactive, render};
use core::bridge::{BoaOpaque, TurJobExecutor, TurModuleLoader};
use core::capability::{Capability, CapabilityDecls};
use core::element::{ElementNodeId, NodeId};
use core::elements::AnyElement;
use core::fonts::FontLoader;
use core::plugin::{Plugin, PluginContext};
use core::js_value::IntoJs;
use core::render::Renderer;

#[cfg(feature = "trace")]
use core::elements::NodeTreeData;

pub struct TurApp {
    boa_context: RefCell<Context>,
    internal: TurAppInternal,
    executor: Rc<TurJobExecutor>,
    /// Autonomous-loop driver. `None` until [`Self::start`] is called
    /// (production); tests leave it unset and pump via [`Self::run_frame`].
    driver: RefCell<Option<Rc<dyn LoopDriver>>>,
    /// Long-lived wake trampoline created in [`Self::start`]: upgrades a
    /// `Weak<Self>` and calls [`Self::wake`]. Held here (and cloned into the
    /// driver via [`LoopDriver::set_wake`]) so it stays alive for the loop's
    /// lifetime; the `Weak` back-ref avoids a reference cycle.
    wake_fn: RefCell<Option<Rc<dyn Fn()>>>,
    /// Embedder-installed callback fired after each autonomous frame — used by
    /// the wasm embedder for DOM side-effects (file-pick resolution, textarea
    /// focus / caret positioning). `None` in tests.
    after_frame: RefCell<Option<AfterFrameHook>>,
}

/// Per-frame hook fired at the end of [`TurApp::wake`] (after `run_frame`,
/// before rescheduling). See [`TurApp::set_after_frame_hook`].
pub type AfterFrameHook = Rc<dyn Fn(FrameOutcome)>;

impl TurApp {
    pub fn load_js(&self, source: &str) -> Result<(), TurError> {
        tracing::info!("load_js: evaluating bundle ({} bytes)", source.len());
        let mut boa = self.boa_context.borrow_mut();
        boa.eval(Source::from_bytes(source).with_path(Path::new("bundle.js")))
            .map_err(|e| {
                tracing::error!("JS eval error: {e}");
                TurError::JsEval(e)
            })?;
        if let Err(e) = self.executor.drain(&mut boa) {
            tracing::error!("load_js drain error: {e}");
        }
        tracing::info!("load_js: bundle evaluated successfully");
        Ok(())
    }

    pub fn load_module(&self, source: &str) -> Result<(), TurError> {
        tracing::info!("load_module: evaluating module ({} bytes)", source.len());
        let mut boa = self.boa_context.borrow_mut();
        let module = boa_engine::Module::parse(
            Source::from_bytes(source).with_path(Path::new("entry.mjs")),
            None,
            &mut boa,
        )
        .map_err(|e| {
            tracing::error!("module parse error: {e}");
            TurError::JsEval(e)
        })?;
        let _promise = module.load_link_evaluate(&mut boa);
        if let Err(e) = boa.run_jobs() {
            tracing::error!("module run_jobs error: {e}");
        }
        drop(boa);
        if let Err(e) = self.executor.drain(&mut self.boa_context.borrow_mut()) {
            tracing::error!("load_module drain error: {e}");
        }
        Ok(())
    }

    pub fn eval_module(&self, source: &str) -> Result<(), TurError> {
        let mut boa = self.boa_context.borrow_mut();
        let module = boa_engine::Module::parse(
            Source::from_bytes(source).with_path(Path::new("eval.mjs")),
            None,
            &mut boa,
        )
        .map_err(|e| {
            tracing::error!("eval_module parse error: {e}");
            TurError::JsEval(e)
        })?;
        let _promise = module.load_link_evaluate(&mut boa);
        if let Err(e) = boa.run_jobs() {
            tracing::error!("eval_module run_jobs error: {e}");
        }
        drop(boa);
        let _ = self.executor.drain(&mut self.boa_context.borrow_mut());
        Ok(())
    }

    pub fn eval_js(&self, source: &str) -> Result<String, TurError> {
        let result = {
            let mut boa = self.boa_context.borrow_mut();
            boa.eval(Source::from_bytes(source))
                .map_err(TurError::JsEval)?
        };
        let s = result
            .as_string()
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|| result.display().to_string());
        Ok(s)
    }

    /// Advance exactly one frame: run the engine's fixed-point flush (events,
    /// reactive updates, layout, microtasks, async polling) and render if
    /// anything changed. Returns the outcome including how the next frame
    /// should be scheduled.
    ///
    /// This is the low-level frame primitive. Embedders normally drive the
    /// engine via [`Self::start`] (autonomous loop); test harnesses and
    /// advanced embedders call this directly.
    ///
    /// Unlike the old `spawn_loop_once`, this takes no time argument — the
    /// clock is the engine's own `Clock` (a real wall-clock in production,
    /// a `FixedClock` the harness advances in tests).
    pub fn run_frame(&self) -> Result<core::app::FrameOutcome, TurError> {
        let mut boa = self.boa_context.borrow_mut();
        self.internal.flush(&mut boa)
    }

    pub fn with_boa_context<R>(&self, f: impl FnOnce(&mut Context) -> R) -> R {
        f(&mut self.boa_context.borrow_mut())
    }

    /// Push a platform (input) event from the embedder — resize, pointer,
    /// wheel, key, IME, or paste. These are dispatched to handlers via
    /// [`AppHandler::handle_platform_event`](core::handler::AppHandler::handle_platform_event).
    /// Also re-arms an idle autonomous loop (see [`Self::start`]) so the event
    /// is processed on the next frame.
    pub fn push_platform_event(&self, event: core::event::PlatformEvent) {
        self.internal
            .app_context
            .borrow_mut()
            .platform_event_queue
            .push(event);
        self.request_wakeup();
    }

    /// Push an engine-internal event onto the app-event bus (programmatic
    /// scrolls, clipboard writes). Most embedders only need
    /// [`Self::push_platform_event`] / [`Self::request_paint`]; this is
    /// exposed for host-initiated app events and testing. Re-arms an idle
    /// autonomous loop like [`Self::push_platform_event`].
    pub fn push_app_event(&self, event: core::event::AppEvent) {
        self.internal
            .app_context
            .borrow_mut()
            .app_event_queue
            .push(event);
        self.request_wakeup();
    }

    /// Request a paint on the next frame. Sets the `need_paint` flag directly
    /// (no event is enqueued), which the flush loop turns into a re-layout +
    /// re-render. Re-arms an idle autonomous loop so the request is processed
    /// even when nothing else is pending (see [`Self::start`]). Used by
    /// embedders after loading JS and by tests asserting an explicit paint.
    pub fn request_paint(&self) {
        self.internal.js_context.need_paint.set(true);
        self.request_wakeup();
    }

    /// Begin autonomous operation: the engine schedules its own frames via
    /// `driver`. The driver fires the engine's wake trampoline when due;
    /// each wake runs one [`Self::run_frame`], the [`Self::after_frame`] hook,
    /// then requests the next wake-up per the frame outcome. Input pushed
    /// via [`Self::push_platform_event`] re-arms an idle loop automatically.
    ///
    /// Must be called exactly once, after JS is loaded. The engine holds a
    /// `Weak` back-reference (no reference cycle), so the loop stops when the
    /// last `Rc<TurApp>` is dropped.
    pub fn start(self: &Rc<Self>, driver: Rc<dyn LoopDriver>) {
        let wake_fn: Rc<dyn Fn()> = {
            let weak = Rc::downgrade(self);
            Rc::new(move || {
                if let Some(app) = weak.upgrade() {
                    app.wake();
                }
            })
        };
        driver.set_wake(wake_fn.clone());
        *self.wake_fn.borrow_mut() = Some(wake_fn);
        *self.driver.borrow_mut() = Some(driver);
        // Kick off frame 1.
        self.wake();
    }

    /// Install a callback fired after each autonomous frame (in [`Self::wake`],
    /// after `run_frame`, before rescheduling). The wasm embedder uses it for
    /// DOM side-effects (file-pick resolution, textarea focus / caret
    /// positioning). Has no effect for manually-pumped (test) operation.
    pub fn set_after_frame_hook(&self, hook: Option<Rc<dyn Fn(FrameOutcome)>>) {
        *self.after_frame.borrow_mut() = hook;
    }

    /// Re-arm an idle autonomous loop: ask the driver for one wake-up on the
    /// next frame. No-op when no driver is installed (tests) or when a frame
    /// is already pending — the driver treats `request_next` as idempotent.
    fn request_wakeup(&self) {
        if let Some(driver) = self.driver.borrow().as_ref() {
            driver.request_next(core::app::NextFrame::Vsync);
        }
    }

    /// One autonomous-frame tick: `run_frame`, the `after_frame` hook, then
    /// reschedule via the driver. Called by the wake trampoline the driver
    /// fires (and by [`Self::start`] for the first frame).
    fn wake(self: &Rc<Self>) {
        let outcome = match self.run_frame() {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("frame loop run_frame error: {e}");
                return;
            }
        };
        if let Some(hook) = self.after_frame.borrow().as_ref() {
            hook.clone()(outcome);
        }
        let next = outcome.schedule;
        if let Some(driver) = self.driver.borrow().as_ref().cloned() {
            driver.request_next(next);
        }
    }

    pub fn dev_tool_element_tree(&self) -> Option<core::elements::DevNodeData> {
        let tree = self.internal.js_context.element_tree.borrow();
        let root_id = tree.root_element_id()?;
        tree.dev_tool_node(root_id.into())
    }

    pub fn dev_tool_get_element(
        &self,
        id: core::element::NodeId,
    ) -> Option<core::elements::DevNodeData> {
        self.internal.js_context.element_tree.borrow().dev_tool_node(id)
    }

    pub fn query_element(&self, key: &[&str]) -> Option<NodeId> {
        self.internal
            .js_context
            .element_tree
            .borrow()
            .query_element(key)
    }

    pub fn focused_element(&self) -> Option<ElementNodeId> {
        self.internal.js_context.focus_manager.borrow().focused()
    }

    pub fn with_element<R>(
        &self,
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R,
    ) -> Option<R> {
        let tree = self.internal.js_context.element_tree.borrow();
        let node = tree.get_element(id)?;
        let element = node.element.as_ref()?;
        Some(cb(element))
    }

    pub fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let focused_id = self.focused_element()?;
        let tree = self.internal.js_context.element_tree.borrow();

        let mut abs_x = 0.0f64;
        let mut abs_y = 0.0f64;
        let mut current: Option<NodeId> = Some(focused_id.into());
        while let Some(id) = current {
            let node = tree.get_element(ElementNodeId::new(id.as_u64()))?;
            abs_x += node.computed_layout.offset.x;
            abs_y += node.computed_layout.offset.y;
            current = node.parent;
        }

        let node = tree.get_element(focused_id)?;
        let element = node.element.as_ref()?;
        let (cx, cy, cw, ch) = element.cursor_rect_relative()?;

        Some((
            abs_x + cx,
            abs_y + cy,
            cw,
            ch,
        ))
    }

    /// True if the currently-focused element is an editable text element.
    /// Used by embedders (e.g. tur-wasm) to manage IME state.
    pub fn focused_is_editable(&self) -> bool {
        use core::focus::helper;
        let tree = self.internal.js_context.element_tree.borrow();
        let focus = self.internal.js_context.focus_manager.borrow();
        helper::focused_is_editable(&tree, &focus)
    }

    #[cfg(feature = "trace")]
    pub fn element_tree(&self) -> std::cell::Ref<'_, NodeTreeData> {
        self.internal.js_context.element_tree.borrow()
    }

    pub fn render_to_pixels(&self) -> Option<Vec<u8>> {
        self.internal.app_context.borrow_mut().render_to_pixels()
    }
}

/// Autonomous-loop driver — the platform scheduling primitive the engine
/// uses to wake itself for the next frame. Implementations live in the
/// embedder: a wasm driver backed by `requestAnimationFrame` / `setTimeout`
/// for the wake trampoline (tur-wasm), or any other platform's wake mechanism.
/// Tests do not install one (they pump [`TurApp::run_frame`] manually).
pub trait LoopDriver {
    /// Install the engine's wake trampoline. The driver must call it exactly
    /// once whenever a wake-up requested via [`Self::request_next`] becomes
    /// due. Set once at [`TurApp::start`].
    fn set_wake(&self, wake: Rc<dyn Fn()>);

    /// Request the next wake-up, replacing any pending request.
    /// - [`NextFrame::Vsync`](core::app::NextFrame) → wake on the next display
    ///   frame (~16 ms).
    /// - [`NextFrame::After(d)`](core::app::NextFrame) → wake after `d`.
    /// - [`NextFrame::Idle`](core::app::NextFrame) → cancel any pending
    ///   wake-up; the loop stops until [`TurApp::push_platform_event`] (via
    ///   `request_next(Vsync)`) re-arms it.
    fn request_next(&self, next: core::app::NextFrame);
}

pub struct TurEngine;

impl TurEngine {
    pub fn builder() -> TurEngineBuilder {
        TurEngineBuilder::new()
    }
}

/// Deferred capability-insert closure. Captures the typed capability value
/// and the static type parameter, so the actual `Capabilities::insert::<C>`
/// call (which requires a static `C`) happens once inside `build()` rather
/// than at registration time. We store closures instead of
/// `Vec<(TypeId, Box<dyn Any>)>` because `Box<dyn Any>` can't be cloned —
/// the closure pattern sidesteps that limitation.
type CapabilityInsert = Box<dyn FnOnce(&core::capability::Capabilities)>;

pub struct TurEngineBuilder {
    renderer: Option<Box<dyn Renderer>>,
    font_loader: Option<Box<dyn FontLoader>>,
    clock: Option<Rc<dyn Clock>>,
    plugins: Vec<Box<dyn Plugin>>,
    capabilities: Vec<CapabilityInsert>,
}

impl Default for TurEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TurEngineBuilder {
    pub fn new() -> Self {
        Self {
            renderer: None,
            font_loader: None,
            clock: None,
            plugins: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    pub fn renderer(mut self, renderer: Box<dyn Renderer>) -> Self {
        self.renderer = Some(renderer);
        self
    }

    pub fn font_loader(mut self, font_loader: Box<dyn FontLoader>) -> Self {
        self.font_loader = Some(font_loader);
        self
    }

    /// Provide the engine clock — the single source of time read by JS
    /// `Date.now()` and timer scheduling, and by paint-time effects via
    /// `PaintContext::now()`. Shared between the boa `Context` and the
    /// engine `Shell`. Required.
    ///
    /// Production passes an [`StdClock`] (real wall clock — `Date.now()` is
    /// live, no manual advancement). Tests pass a [`FixedClock`] they advance
    /// themselves frame-by-frame.
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
    /// Useful when a caller wants to mix plugin types into a single collection
    /// (e.g. a test harness aggregating plugins from multiple sources).
    pub fn plugin_boxed(mut self, plugin: Box<dyn Plugin>) -> Self {
        self.plugins.push(plugin);
        self
    }

    /// Register a capability (a plugin-swappable backend) so it's available
    /// to plugins (via [`PluginContext::capability`]), bridge fns (via
    /// `extract_ctx(...).capability()`), handlers (via
    /// [`HandlerContext::capabilities`](core::handler::HandlerContext::capabilities)),
    /// and the engine itself.
    ///
    /// Capabilities are inserted into the registry before any plugin's
    /// `register` runs, so plugin call-site order (`capability` before/after
    /// `plugin`) doesn't matter.
    ///
    /// Plugins declare hard dependencies via [`Plugin::requires`]; the engine
    /// validates those before any plugin side effects.
    pub fn capability<C: Capability>(mut self, cap: C) -> Self {
        self.capabilities
            .push(Box::new(move |registry: &core::capability::Capabilities| {
                registry.insert::<C>(cap);
            }));
        self
    }

    pub fn build(self) -> Result<Rc<TurApp>, TurError> {
        let renderer = self.renderer.expect("renderer must be set");
        let font_loader = self
            .font_loader
            .expect("font_loader must be set");
        let clock = self
            .clock
            .expect("clock must be set (use TurEngineBuilder::clock)");

        // boa's `ContextBuilder::clock<C: Clock + 'static>` is generic over a
        // concrete (`Sized`) `C`, so it won't accept an already-erased
        // `Rc<dyn Clock>`. `ClockProxy` is a Sized adapter that delegates to
        // the shared `Rc<dyn Clock>` — giving boa and the engine `Shell` (and,
        // for `FixedClock`, the test harness's `forward` calls) one shared
        // time source.
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

        let executor = Rc::new(TurJobExecutor::new());
        let module_loader = TurModuleLoader::new();
        let mut boa_context = Context::builder()
            .clock(Rc::new(ClockProxy(clock.clone())))
            .job_executor(executor.clone())
            .module_loader(module_loader.clone())
            .build()
            .expect("failed to build boa context");

        let mut internal = TurAppInternal::new(
            renderer,
            font_loader,
            executor.clone(),
            clock,
        );

        let opaque = BoaOpaque::new(internal.js_context.clone(), &mut boa_context);
        let ctx_val: boa_engine::JsValue = opaque.object().clone().into();

        // Engine-owned `viewportSize$` reactive source. Created here (needs
        // `&mut Context` for the initial `{width,height}` value + the opaque
        // wrap) and synced each frame in `TurAppInternal::flush`. The handle
        // (`Source<JsValue>`, a `Copy` `AtomId`) lives on `internal`; the
        // `JsValue` opaque is handed to plugins so `std` can export it as
        // the `viewportSize$` const in `builtin:tur/std`.
        let viewport_size_js: boa_engine::JsValue = {
            let (w, h) = internal.app_context.borrow().size;
            let init = TurAppInternal::viewport_js(&mut boa_context, w, h);
            let src: core::reactive::Source<boa_engine::JsValue> =
                internal.js_context.store.bridge().source(init);
            internal.viewport_size = Some(src);
            internal.last_viewport.set((w, h));
            src.into_js(&mut boa_context)
        };

        let mut core_fns: Vec<FnEntry> = Vec::new();
        core_fns.extend(reactive::fns());
        core_fns.extend(render::fns());
        let core_module = build_native_module(
            &mut boa_context,
            opaque.object().clone().into(),
            &core_fns,
            &[],
            &[],
        );
        module_loader.register("builtin:tur/core", core_module);

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
        let _ = boa_context
            .register_global_property(js_string!("turDevTool"), dt_obj, Attribute::all());

        console::register_console_globals(&mut boa_context);

        // Insert every builder-level capability into the shared registry
        // BEFORE plugins run, so plugin call-site order (`capability` before
        // or after `plugin`) doesn't matter. Each closure calls
        // `Capabilities::insert::<C>(cap)` with the original typed value.
        for insert_fn in self.capabilities {
            insert_fn(&internal.js_context.capabilities);
        }

        // Validate every plugin's `requires` declaration against the registry
        // BEFORE any plugin's `register` runs side effects. A missing cap
        // fails fast with a clear message naming the missing type and the fix.
        {
            let mut decls = CapabilityDecls::new();
            for plugin in &self.plugins {
                plugin.requires(&mut decls);
            }
            for (cap_id, cap_name) in decls.iter() {
                if !internal.js_context.capabilities.contains_id(cap_id) {
                    return Err(TurError::Other(format!(
                        "plugin requires capability `{cap_name}` which is not registered; \
                         add `.capability({cap_name}::new(...))` to the engine builder"
                    )));
                }
            }
        }

        for plugin in &self.plugins {
            let mut plugin_ctx = PluginContext {
                boa: &mut boa_context,
                loader: module_loader.clone(),
                js_ctx_value: ctx_val.clone(),
                js_ctx: internal.js_context.clone(),
                app: internal.app_context.clone(),
                subsystems: internal.subsystems.clone(),
                viewport_size: viewport_size_js.clone(),
            };
            plugin.register(&mut plugin_ctx)?;
        }

        // Install the cursor backend on the Shell from the capability
        // registry (registered by the embedder via `.capability(CursorCap::new(...))`).
        // Fall back to `NoopCursor` when no cursor capability is present —
        // cursor never changes, but the engine still runs.
        {
            let cursor_backend = internal
                .js_context
                .capability()
                .of::<core::platform::CursorCap>()
                .map(|c| c.backend().clone())
                .unwrap_or_else(|| Rc::new(std::cell::RefCell::new(core::platform::NoopCursor)));
            internal
                .app_context
                .borrow_mut()
                .shell
                .set_cursor_platform(cursor_backend);
        }

        tracing::info!(
            "TurApp initialized ({} plugins)",
            self.plugins.len()
        );

        Ok(Rc::new(TurApp {
            boa_context: RefCell::new(boa_context),
            internal,
            executor,
            driver: RefCell::new(None),
            wake_fn: RefCell::new(None),
            after_frame: RefCell::new(None),
        }))
    }
}
