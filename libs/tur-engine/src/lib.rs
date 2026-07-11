pub mod core;
pub mod elements;
pub mod renderer;

pub mod error;

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use boa_engine::context::time::FixedClock;
use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::Context;
use boa_engine::NativeFunction;
use boa_engine::Source;

use error::TurError;

use core::app::TurAppInternal;
use core::async_::AsyncRuntime;
use core::bridge::helpers::FnEntry;
use core::bridge::module_loader::{build_fn_module, build_native_module, bound_native};
use core::bridge::{console, dev_tool, reactive, render, timer};
use core::bridge::{BoaOpaque, TurJobExecutor, TurModuleLoader, TimerState};
use core::element::{ElementNodeId, NodeId};
use core::elements::AnyElement;
use core::fonts::FontLoader;
use core::plugin::{Plugin, PluginContext};
use core::js_value::IntoJs;
use core::render::Renderer;

#[cfg(feature = "trace")]
use core::elements::NodeTreeData;

pub struct TurApp {
    boa_context: Context,
    internal: TurAppInternal,
    executor: Rc<TurJobExecutor>,
    module_loader: Rc<TurModuleLoader>,
}

impl TurApp {
    pub fn load_js(&mut self, source: &str) -> Result<(), TurError> {
        tracing::info!("load_js: evaluating bundle ({} bytes)", source.len());
        self.boa_context
            .eval(Source::from_bytes(source).with_path(Path::new("bundle.js")))
            .map_err(|e| {
                tracing::error!("JS eval error: {e}");
                TurError::JsEval(e)
            })?;
        if let Err(e) = self.executor.drain(&mut self.boa_context) {
            tracing::error!("load_js drain error: {e}");
        }
        tracing::info!("load_js: bundle evaluated successfully");
        Ok(())
    }

    pub fn load_module(&mut self, source: &str) -> Result<(), TurError> {
        tracing::info!("load_module: evaluating module ({} bytes)", source.len());
        let module = boa_engine::Module::parse(
            Source::from_bytes(source).with_path(Path::new("entry.mjs")),
            None,
            &mut self.boa_context,
        )
        .map_err(|e| {
            tracing::error!("module parse error: {e}");
            TurError::JsEval(e)
        })?;
        let _promise = module.load_link_evaluate(&mut self.boa_context);
        if let Err(e) = self.boa_context.run_jobs() {
            tracing::error!("module run_jobs error: {e}");
        }
        if let Err(e) = self.executor.drain(&mut self.boa_context) {
            tracing::error!("load_module drain error: {e}");
        }
        Ok(())
    }

    pub fn eval_module(&mut self, source: &str) -> Result<(), TurError> {
        let module = boa_engine::Module::parse(
            Source::from_bytes(source).with_path(Path::new("eval.mjs")),
            None,
            &mut self.boa_context,
        )
        .map_err(|e| {
            tracing::error!("eval_module parse error: {e}");
            TurError::JsEval(e)
        })?;
        let _promise = module.load_link_evaluate(&mut self.boa_context);
        if let Err(e) = self.boa_context.run_jobs() {
            tracing::error!("eval_module run_jobs error: {e}");
        }
        let _ = self.executor.drain(&mut self.boa_context);
        Ok(())
    }

    pub fn eval_js(&mut self, source: &str) -> Result<String, TurError> {
        let result = self
            .boa_context
            .eval(Source::from_bytes(source))
            .map_err(TurError::JsEval)?;
        let s = result
            .as_string()
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|| result.display().to_string());
        Ok(s)
    }

    /// Register a synthetic ES module under `specifier` whose exports are the
    /// given native functions. Embedders (tur-wasm) use this to expose host
    /// services as importable modules — e.g. `builtin:tur/host`.
    pub fn register_host_module(
        &mut self,
        specifier: &str,
        exports: Vec<(String, boa_engine::NativeFunction, usize)>,
    ) -> Result<(), boa_engine::JsError> {
        let owned: Vec<(&str, boa_engine::NativeFunction, usize)> = exports
            .iter()
            .map(|(n, f, l)| (n.as_str(), f.clone(), *l))
            .collect();
        let module = build_fn_module(&mut self.boa_context, &owned);
        self.module_loader.register(specifier, module);
        tracing::info!("registered host module {specifier} ({} exports)", owned.len());
        Ok(())
    }

    pub fn spawn_loop_once(&mut self, advanced_time: Duration) -> Result<(), TurError> {
        self.internal
            .app_context
            .borrow()
            .shell
            .forward(advanced_time.as_millis() as u64);
        self.internal.flush(&mut self.boa_context)?;
        Ok(())
    }

    pub fn with_boa_context<R>(&mut self, f: impl FnOnce(&mut Context) -> R) -> R {
        f(&mut self.boa_context)
    }

    pub fn push_event(&self, event: core::event::AppEvent) {
        self.internal
            .app_context
            .borrow_mut()
            .event_queue
            .push(event);
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

    pub fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        self.internal.app_context.borrow_mut().render_to_pixels()
    }
}

pub struct TurEngine;

impl TurEngine {
    pub fn builder() -> TurEngineBuilder {
        TurEngineBuilder::new()
    }
}

type HostExports = Vec<(String, NativeFunction, usize)>;

pub struct TurEngineBuilder {
    renderer: Option<Box<dyn Renderer>>,
    font_loader: Option<Box<dyn FontLoader>>,
    async_runtime: Option<Rc<dyn AsyncRuntime>>,
    plugins: Vec<Box<dyn Plugin>>,
    host_modules: Vec<(String, HostExports)>,
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
            async_runtime: None,
            plugins: Vec::new(),
            host_modules: Vec::new(),
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

    /// Provide the async runtime (wall-clock source for the engine-owned
    /// [`AsyncExecutor`]). Required — every backend must supply one:
    /// `WasmRuntime` for wasm (`Performance::now()`), `TestRuntime` for
    /// integration tests (deterministic clock).
    pub fn async_runtime(mut self, runtime: Rc<dyn AsyncRuntime>) -> Self {
        self.async_runtime = Some(runtime);
        self
    }

    pub fn plugin<P: Plugin + 'static>(mut self, plugin: P) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    pub fn host_module(
        mut self,
        specifier: impl Into<String>,
        exports: HostExports,
    ) -> Self {
        self.host_modules.push((specifier.into(), exports));
        self
    }

    pub fn build(self) -> Result<TurApp, TurError> {
        let renderer = self.renderer.expect("renderer must be set");
        let font_loader = self
            .font_loader
            .expect("font_loader must be set");
        let async_runtime = self
            .async_runtime
            .expect("async_runtime must be set (use TurEngineBuilder::async_runtime)");

        let clock = Rc::new(FixedClock::from_millis(0));
        let executor = Rc::new(TurJobExecutor::new());
        let module_loader = TurModuleLoader::new();
        let mut boa_context = Context::builder()
            .clock(clock.clone())
            .job_executor(executor.clone())
            .module_loader(module_loader.clone())
            .build()
            .expect("failed to build boa context");

        let mut internal = TurAppInternal::new(
            renderer,
            font_loader,
            executor.clone(),
            clock,
            async_runtime,
        );

        let opaque = BoaOpaque::new(internal.js_context.clone(), &mut boa_context);
        let ctx_val: boa_engine::JsValue = opaque.object().clone().into();

        // Engine-owned `viewportSize$` reactive source. Created here (needs
        // `&mut Context` for the initial `{width,height}` value + the opaque
        // wrap) and synced each frame in `TurAppInternal::flush`. The handle
        // (`Source<JsValue>`, a `Copy` `AtomId`) lives on `internal`; the
        // `JsValue` opaque is handed to plugins so `tur-std` can export it as
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

        let timer_state = Rc::new(RefCell::new(TimerState::new()));
        timer::register_timer_globals(
            &mut boa_context,
            timer_state,
            internal.needs_draw.clone(),
        );
        console::register_console_globals(&mut boa_context);

        for (specifier, exports) in &self.host_modules {
            let owned: Vec<(&str, NativeFunction, usize)> = exports
                .iter()
                .map(|(n, f, l)| (n.as_str(), f.clone(), *l))
                .collect();
            let module = build_fn_module(&mut boa_context, &owned);
            module_loader.register(specifier, module);
            tracing::info!("registered host module {specifier} ({} exports)", owned.len());
        }

        for plugin in &self.plugins {
            let mut plugin_ctx = PluginContext {
                boa: &mut boa_context,
                loader: module_loader.clone(),
                js_ctx_value: ctx_val.clone(),
                js_ctx: internal.js_context.clone(),
                app: internal.app_context.clone(),
                needs_draw: internal.needs_draw.clone(),
                async_executor: internal.async_executor.clone(),
                viewport_size: viewport_size_js.clone(),
            };
            plugin.register(&mut plugin_ctx)?;
            if let Some(f) = plugin.cursor_output() {
                internal.app_context.borrow_mut().shell.set_cursor_output(f);
            }
        }

        tracing::info!("TurApp initialized ({} plugins)", self.plugins.len());

        Ok(TurApp {
            boa_context,
            internal,
            executor,
            module_loader,
        })
    }
}
