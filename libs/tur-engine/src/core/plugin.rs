use std::cell::Cell;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use boa_engine::context::time::Clock;
use boa_engine::js_string;
use boa_engine::property::Attribute;
use boa_engine::class::Class;
use boa_engine::Context;
use boa_engine::JsError;
use boa_engine::JsValue;
use boa_engine::Module;
use boa_engine::NativeFunction;
use boa_engine::Source;

use crate::core::app::TurAppContext;
use crate::core::js_runtime::helpers::{ConstEntry, FnEntry};
use crate::core::js_runtime::module_loader::{build_fn_module, build_native_module};
use crate::core::js_runtime::{TurJsContext, TurModuleLoader};
use crate::core::capability::{Capabilities, CapabilityDecls};
use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::subsystem::Subsystem;
use crate::error::TurError;
/// A plugin that extends the engine with elements, bridge modules, subsystems,
/// and/or platform capabilities.
///
/// Each plugin is registered via [`TurEngineBuilder::plugin`](crate::TurEngineBuilder::plugin)
/// and installed once during `build()`. The [`register`](Plugin::register)
/// method is the build-time entry point — it receives a [`PluginContext`]
/// exposing all registration primitives (modules, subsystems, classes, globals).
///
/// Plugins declare hard-required capabilities via
/// [`requires`](Plugin::requires); the engine builder validates every
/// declaration against the registered capabilities before any plugin's
/// `register` runs, so a missing capability fails fast at `build()` with a
/// clear error (naming the missing type and the fix) instead of midway
/// through side-effecting registration.
pub trait Plugin {
    /// Declare capabilities this plugin hard-requires. Called by the engine
    /// builder BEFORE any plugin's `register` runs. If a declared capability
    /// is missing, `build()` returns `TurError::Other(...)` naming the
    /// missing type.
    ///
    /// Default: no requirements. Optional capabilities should NOT be declared
    /// here — the plugin should look them up via
    /// [`PluginContext::capability`] in `register` and handle absence
    /// gracefully.
    fn requires(&self, _decls: &mut CapabilityDecls) {}

    /// Build-time registration. Called once during `build()` after capability
    /// validation. Register modules, handlers, classes, globals via
    /// `ctx.register_*()`.
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError>;
}

/// Build-time context passed to [`Plugin::register`]. Only available during
/// `build()` — after the app is constructed, no further registration is possible.
///
/// Exposes registration primitives for JS modules, boa classes, subsystems,
/// and global properties. Each `register_*` method is self-contained: call them
/// sequentially within `register.
pub struct PluginContext<'a> {
    pub(crate) boa: &'a mut Context,
    pub(crate) loader: Rc<TurModuleLoader>,
    pub js_ctx_value: JsValue,
    pub(crate) js_ctx: TurJsContext,
    pub(crate) app: Rc<RefCell<TurAppContext>>,
    /// Plugin-registered flush subsystems. Shared with
    /// [`TurAppInternal::subsystems`](crate::core::app::TurAppInternal) —
    /// plugins push here, the engine iterates the same vec during flush.
    pub(crate) subsystems: Rc<RefCell<Vec<Box<dyn Subsystem>>>>,
    /// Engine-owned `viewportSize$` source handle (a `JsValue` opaque wrapping
    /// a `Source<JsValue>`). Plugins export this as a const so JS can
    /// `import { viewportSize$ } from "tur:std"` and read the live
    /// canvas size via `get`. The engine updates it each frame in `flush`.
    pub viewport_size: JsValue,
}

impl<'a> PluginContext<'a> {
    /// Register a ctx-bound native module (bridge fns that receive `TurJsContext`
    /// as their first argument) plus optional free-form closure exports. Used
    /// for `tur:std` and similar.
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
    /// Used for `tur:net`, `tur-ext/demo-helper`, etc.
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

    /// Cheaply-cloned view over the capability registry. Plugins call
    /// `ctx.capability().of::<C>()` to look up sibling capabilities (or
    /// `require::<C>()` for a hard error). Returns a fresh `Capabilities`
    /// handle (single `Rc` bump).
    pub fn capability(&self) -> Capabilities {
        self.js_ctx.capability()
    }

    /// Access the boa `Context` directly (for custom registration needs).
    pub fn boa_mut(&mut self) -> &mut Context {
        self.boa
    }

    /// The `need_paint` flag — setting it triggers a re-layout on the next frame.
    pub fn need_paint(&self) -> &Rc<Cell<bool>> {
        &self.js_ctx.need_paint
    }

    /// The engine-wide mutation queue (shared with `flush_pending_mutations`).
    /// Plugins that defer JS callbacks (e.g. animation `onTick`/`onEnd`) stash
    /// this handle at registration time and push onto the queue when their
    /// subsystem ticks.
    pub fn mutation_queue(&self) -> Rc<RefCell<PendingMutationInvocationQueue>> {
        self.js_ctx.mutation_queue.clone()
    }

    /// The engine's shared clock (set via
    /// [`TurEngineBuilder::clock`](crate::TurEngineBuilder::clock)). Plugins
    /// that own time-driven subsystems (animation, audio, etc.) stash this
    /// handle at registration time and query `clock.now()` during their tick.
    pub fn clock(&self) -> Rc<dyn Clock> {
        self.app.borrow().shell.clock()
    }

    /// Register a [`Subsystem`] — a long-lived participant in the engine's
    /// per-frame `flush` loop. Subsystems tick once per frame (not once per
    /// fixed-point iteration), in registration order. See the
    /// [`subsystem`](crate::core::subsystem) module docs for details.
    pub fn register_subsystem(&mut self, sub: Box<dyn Subsystem>) {
        self.subsystems.borrow_mut().push(sub);
    }

    /// Register a JS source module under a bare specifier (e.g.
    /// `tur:animation`). The source is parsed into a boa `Module` and
    /// stored in the loader; consumer code can then
    /// `import { ... } from "<specifier>"` and boa resolves it to this module.
    ///
    /// Used by plugins that ship their own JS alongside the Rust bridge fns
    /// (e.g. `tur-animation` ships an `index.js` defining `AnimatedContainer`
    /// etc. on top of native bridge fns registered via
    /// [`register_module`](Self::register_module)).
    pub fn register_js_module(
        &mut self,
        specifier: &str,
        source: &str,
        path: &Path,
    ) -> Result<(), TurError> {
        let module = Module::parse(
            Source::from_bytes(source).with_path(path),
            None,
            self.boa,
        )
        .map_err(|e| TurError::Other(format!("failed to parse JS module {specifier}: {e}")))?;
        self.loader.register(specifier, module);
        tracing::info!("registered JS module {specifier} ({} bytes)", source.len());
        Ok(())
    }
}
