use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::js_string;
use boa_engine::property::Attribute;
use boa_engine::class::Class;
use boa_engine::Context;
use boa_engine::JsError;
use boa_engine::JsValue;
use boa_engine::NativeFunction;
use tur_shared::Cursor;

use crate::core::app::TurAppContext;
use crate::core::async_::AsyncExecutor;
use crate::core::bridge::helpers::{ConstEntry, FnEntry};
use crate::core::bridge::module_loader::{build_fn_module, build_native_module};
use crate::core::bridge::{TurJsContext, TurModuleLoader};
use crate::core::handler::AppHandler;
use crate::error::TurError;
/// A plugin that extends the engine with elements, bridge modules, handlers,
/// and/or platform capabilities.
///
/// Each plugin is registered via [`TurEngineBuilder::plugin`](crate::TurEngineBuilder::plugin)
/// and installed once during `build()`. The [`register`](Plugin::register) method
/// is the build-time entry point — it receives a [`PluginContext`] exposing all
/// registration primitives (modules, handlers, classes, globals).
///
/// Runtime hooks (like cursor output) are provided via separate optional trait
/// methods so they stay conceptually distinct from one-time registration.
pub trait Plugin {
    /// Build-time registration. Called once during `build()`. Register
    /// modules, handlers, classes, globals via `ctx.register_*()`.
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError>;

    /// Runtime cursor-output callback. The engine calls this once during build
    /// to extract the closure; the closure itself fires at runtime during the
    /// frame loop whenever the resolved cursor changes.
    ///
    /// Default: `None` (most plugins don't produce cursor output).
    fn cursor_output(&self) -> Option<Box<dyn FnMut(Cursor)>> {
        None
    }
}

/// Build-time context passed to [`Plugin::register`]. Only available during
/// `build()` — after the app is constructed, no further registration is possible.
///
/// Exposes registration primitives for JS modules, boa classes, event handlers,
/// and global properties. Each `register_*` method is self-contained: call them
/// sequentially within `register`.
pub struct PluginContext<'a> {
    pub(crate) boa: &'a mut Context,
    pub(crate) loader: Rc<TurModuleLoader>,
    pub js_ctx_value: JsValue,
    pub(crate) js_ctx: TurJsContext,
    pub(crate) app: Rc<RefCell<TurAppContext>>,
    /// Engine-owned async executor. Plugins use this to spawn Rust futures
    /// (clipboard/http work, etc.) — see [`AsyncExecutor::spawn`] and
    /// [`AsyncExecutor::spawn_detached`].
    pub(crate) async_executor: Rc<AsyncExecutor>,
    /// Engine-owned `viewportSize$` source handle (a `JsValue` opaque wrapping
    /// a `Source<JsValue>`). Plugins export this as a const so JS can
    /// `import { viewportSize$ } from "builtin:tur/std"` and read the live
    /// canvas size via `get`. The engine updates it each frame in `flush`.
    pub viewport_size: JsValue,
}

impl<'a> PluginContext<'a> {
    /// Register a ctx-bound native module (bridge fns that receive `TurJsContext`
    /// as their first argument) plus optional free-form closure exports. Used
    /// for `builtin:tur/std` and similar.
    ///
    /// `closures` is for bridge fns that capture state which can't live on
    /// `TurJsContext` (e.g. a `Clipboard` impl provided by a plugin). Each
    /// closure is registered as-is, with no ctx binding.
    pub fn register_module(
        &mut self,
        specifier: &str,
        fns: Vec<FnEntry>,
        closures: Vec<(&str, usize, NativeFunction)>,
        consts: Vec<ConstEntry>,
    ) {
        let module =
            build_native_module(self.boa, self.js_ctx_value.clone(), &fns, &closures, &consts);
        self.loader.register(specifier, module);
        tracing::info!(
            "registered module {specifier} ({} fns, {} closures, {} consts)",
            fns.len(),
            closures.len(),
            consts.len()
        );
    }

    /// Register a ctx-free native module (host fns that don't need `TurJsContext`).
    /// Used for `builtin:tur/host`, `builtin:tur/net`, etc.
    pub fn register_host_module(
        &mut self,
        specifier: &str,
        exports: Vec<(String, NativeFunction, usize)>,
    ) {
        let owned: Vec<(&str, NativeFunction, usize)> = exports
            .iter()
            .map(|(n, f, l)| (n.as_str(), f.clone(), *l))
            .collect();
        let module = build_fn_module(self.boa, &owned);
        self.loader.register(specifier, module);
        tracing::info!("registered host module {specifier} ({} exports)", owned.len());
    }

    /// Register a boa `JsData` global class (e.g. `TextEditingController`).
    pub fn register_class<T: Class>(&mut self) -> Result<(), JsError> {
        self.boa.register_global_class::<T>()
    }

    /// Register an [`AppHandler`] for input event dispatch.
    pub fn register_handler(&mut self, handler: Box<dyn AppHandler>) {
        self.app.borrow_mut().register_handler(handler);
    }

    /// Register a global JS property on `globalThis`.
    pub fn register_global(&mut self, name: &str, value: JsValue) {
        let _ = self
            .boa
            .register_global_property(js_string!(name), value, Attribute::all());
    }

    /// Access the shared JS context (reactive store, node tree, etc.).
    pub fn js_ctx(&self) -> &TurJsContext {
        &self.js_ctx
    }

    /// Access the boa `Context` directly (for custom registration needs).
    pub fn boa_mut(&mut self) -> &mut Context {
        self.boa
    }

    /// The `needs_draw` flag — setting it triggers a re-layout on the next frame.
    pub fn needs_draw(&self) -> &Rc<Cell<bool>> {
        &self.js_ctx.needs_draw
    }

    /// The engine-owned async executor. Plugins call `spawn_detached(...)` to
    /// run Rust futures (clipboard/http/etc.); futures push completion
    /// closures via `complete(...)` that settle JsPromises under `&mut
    /// Context` during the next `flush`.
    pub fn async_executor(&self) -> &Rc<AsyncExecutor> {
        &self.async_executor
    }
}
