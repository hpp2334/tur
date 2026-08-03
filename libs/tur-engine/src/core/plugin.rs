use std::cell::Cell;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use boa_engine::Context;
use boa_engine::JsError;
use boa_engine::JsValue;
use boa_engine::Module;
use boa_engine::NativeFunction;
use boa_engine::Source;
use boa_engine::class::Class;
use boa_engine::context::time::Clock;
use boa_engine::js_string;
use boa_engine::property::Attribute;

use crate::core::app::TurAppContext;
use crate::core::capability::{Capabilities, CapabilityDecls};
use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::fonts::FontContext;
use crate::core::js_runtime::helpers::{ConstEntry, FnEntry};
use crate::core::js_runtime::module_loader::{build_fn_module, build_native_module};
use crate::core::js_runtime::{TurJsContext, TurModuleLoader};
use crate::core::subsystem::Subsystem;
use crate::error::TurError;
/// A plugin that extends the engine with elements, bridge modules, subsystems,
/// and/or platform capabilities.
///
/// A plugin is registered on the [`TurRuntime`](crate::TurRuntime) once (via
/// [`TurRuntimeBuilder::plugin`](crate::TurRuntimeBuilder::plugin)). The runtime
/// then drives the plugin through two phases:
///
/// 1. [`compile`](Plugin::compile) — called **once** on the runtime after
///    capabilities are inserted and `requires` is validated. Use it for any
///    one-time, instance-independent work: pre-validating JS module sources,
///    caching descriptor tables, etc. Defaults to a no-op.
///
/// 2. [`register`](Plugin::register) — called **once per instance** (per
///    [`TurRuntime::create_app`](crate::TurRuntime::create_app)) into that
///    instance's fresh boa `Context`. Because `register` takes `&self`, the
///    **same** plugin object is reused across every instance — no factory
///    needed. Stateful per-instance artifacts (subsystems, handles) are created
///    fresh inside `register` and pushed into the per-instance
///    [`PluginContext`].
///
/// Plugins declare hard-required capabilities via
/// [`requires`](Plugin::requires); the runtime validates every declaration
/// against the registered capabilities before any plugin's `compile`/`register`
/// runs, so a missing capability fails fast at runtime build with a clear error
/// (naming the missing type and the fix) instead of midway through
/// side-effecting registration.
/// **Phase 7**: requires `Send + Sync` so plugin config can be shared across
/// worker threads (the runtime hands the plugin vec to whichever worker
/// spawns the instance). Production plugins are zero-field unit structs
/// (trivially `Send + Sync`); test plugins that captured `Rc<RefCell<>>`
/// state or pre-built `NativeFunction`s must migrate to `Arc<Mutex<>>` /
/// builder closures.
pub trait Plugin: Send + Sync {
    /// Declare capabilities this plugin hard-requires. Called by the runtime
    /// builder BEFORE any plugin's `compile`/`register` runs. If a declared
    /// capability is missing, runtime `build()` returns `TurError::Other(...)`
    /// naming the missing type.
    ///
    /// Default: no requirements. Optional capabilities should NOT be declared
    /// here — the plugin should look them up via
    /// [`PluginContext::capability`] in `register` and handle absence
    /// gracefully.
    fn requires(&self, _decls: &mut CapabilityDecls) {}

    /// One-time, runtime-level compilation. Called once after capabilities are
    /// inserted and `requires` is validated, before any instance is created.
    /// Use it to pre-validate JS module sources, build descriptor tables, or
    /// do any work that is identical across every instance. Defaults to a
    /// no-op.
    ///
    /// Note: boa `Module`s are realm-bound (a `Module::parse` needs a
    /// `&mut Context`), so cross-instance sharing of *parsed* modules is not
    /// possible today — the actual parse still happens per instance in
    /// [`register`](Self::register). `compile` is the seam for future
    /// caching and for failing fast on bad module sources at runtime build
    /// time.
    fn compile(&self, _cx: &mut CompileContext) -> Result<(), TurError> {
        Ok(())
    }

    /// Per-instance registration. Called once per
    /// [`TurRuntime::create_app`](crate::TurRuntime::create_app) into a fresh
    /// boa `Context`. Register modules, handlers, classes, globals via
    /// `ctx.register_*()`.
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError>;
}

/// Context passed to [`Plugin::compile`]. Provides read access to the
/// runtime-level shared resources: the capability registry (for ad-hoc
/// validation beyond `requires`) and the shared font context.
pub struct CompileContext<'a> {
    pub capabilities: &'a Capabilities,
    pub font_context: &'a FontContext,
}

/// Per-instance context passed to [`Plugin::register`]. Only available while an
/// instance is being constructed — after the app is built, no further
/// registration is possible.
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
    /// Always-installed event bus — shared with
    /// [`TurAppInternal::event_bus`](crate::core::app::TurAppInternal). Plugins
    /// (specifically `install_event_bus`) read this to wire up the JS bridge
    /// (`eventBus.on`/`send`) and the [`HostBusSubsystem`] against the same
    /// handle that [`TurApp::event_bus`](crate::TurApp::event_bus) returns to
    /// embedders.
    ///
    /// [`HostBusSubsystem`]: crate::core::event_bus::HostBusSubsystem
    pub(crate) event_bus: Rc<crate::core::event_bus::EventBus>,
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
        let module = build_native_module(
            self.boa,
            self.js_ctx_value.clone(),
            &fns,
            &closures,
            &consts,
        );
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
        tracing::info!(
            "registered host module {specifier} ({} exports)",
            owned.len()
        );
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

    /// Worker-thread scheduler. Plugins' bridge fns call
    /// `worker_sched.spawn_local(fut)` (via `Pin::pin(fut).into()` to box
    /// it) to drive async work (clipboard reads, http requests, sleep
    /// futures).
    pub fn worker_sched(&self) -> &Rc<dyn crate::core::scheduler::WorkerScheduler> {
        self.js_ctx.worker_sched()
    }

    /// Cheap-cloned completion handle. Plugins' bridge fns capture this
    /// inside spawned futures and call `push(closure)` to settle
    /// JsPromises under `&mut Context` on the next flush.
    pub fn completion_handle(&self) -> crate::core::async_::CompletionHandle {
        self.js_ctx.completion_handle()
    }

    /// The engine-wide mutation queue (shared with `flush_pending_mutations`).
    /// Plugins that defer JS callbacks (e.g. animation `onTick`/`onEnd`) stash
    /// this handle at registration time and push onto the queue when their
    /// subsystem ticks.
    pub fn mutation_queue(&self) -> Rc<RefCell<PendingMutationInvocationQueue>> {
        self.js_ctx.mutation_queue.clone()
    }

    /// The engine's shared clock (set via
    /// [`TurRuntimeBuilder::clock`](crate::TurRuntimeBuilder::clock)). Plugins
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

    /// The always-installed event bus handle (shared with
    /// [`TurApp::event_bus`](crate::TurApp::event_bus)). Plugins that need to
    /// wire up host↔JS byte traffic (specifically [`install_event_bus`])
    /// read this and clone the `Rc` for their subsystem / bridge captures.
    ///
    /// [`install_event_bus`]: crate::core::event_bus::install_event_bus
    pub fn event_bus(&self) -> Rc<crate::core::event_bus::EventBus> {
        self.event_bus.clone()
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
        let module = Module::parse(Source::from_bytes(source).with_path(path), None, self.boa)
            .map_err(|e| TurError::Other(format!("failed to parse JS module {specifier}: {e}")))?;
        self.loader.register(specifier, module);
        tracing::info!("registered JS module {specifier} ({} bytes)", source.len());
        Ok(())
    }
}
