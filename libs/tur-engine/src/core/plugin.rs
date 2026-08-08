use std::cell::RefCell;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context as TaskContext, Poll};

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
    /// Engine-owned `viewportSize$` source handle (a `JsValue` opaque wraps
    /// a `Source<JsValue>`). Plugins export this as a const so JS can
    /// `import { viewportSize$ } from "tur:std"` and read the live
    /// canvas size via `get`. The engine updates it each frame in `flush`.
    pub viewport_size: JsValue,
    /// The engine's [`AsyncPluginContext`] — a `Send + Sync + Clone`
    /// handle for hopping work onto the engine's main thread (for OS APIs
    /// that require it, e.g. macOS `NSPasteboard` via `arboard`). Set by
    /// the engine when the `PluginContext` is constructed; plugins obtain a
    /// clone via [`to_async`](PluginContext::to_async). Capabilities that
    /// need main-thread access receive their own clone at construction via
    /// [`TurRuntimeBuilder::capability`](crate::TurRuntimeBuilder)'s
    /// closure form.
    pub(crate) async_cx: AsyncPluginContext,
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

    /// Spawn a worker-side async task, handing it an
    /// [`AsyncWorkerContext`](crate::core::async_::AsyncWorkerContext) for
    /// timers / nested spawns / paint signals. Plugins' bridge fns use this
    /// instead of the raw scheduler. See
    /// [`TurJsContext::spawn_local`](crate::core::js_runtime::TurJsContext::spawn_local).
    pub fn spawn_local<F, Fut>(&self, f: F) -> crate::core::scheduler::TaskHandle
    where
        F: FnOnce(crate::core::async_::AsyncWorkerContext) -> Fut,
        Fut: std::future::Future<Output = ()> + 'static,
    {
        self.js_ctx.spawn_local(f)
    }

    /// Obtain the engine's [`AsyncPluginContext`] — a `Send + Sync + Clone`
    /// handle for hopping work onto the engine's main thread. Plugins /
    /// bridges / subsystems that need to run OS-API calls on main (e.g.
    /// macOS `NSPasteboard` via `arboard`) clone this and call
    /// [`AsyncPluginContext::run_on_main`] (sync closure, result bridged
    /// via oneshot) or [`AsyncPluginContext::spawn_on_main`]
    /// (fire-and-forget).
    ///
    /// The hop runs on a serialized drain on the engine's main thread
    /// (safe for non-reentrant OS APIs). The engine creates the channel
    /// internally at `build()` — no embedder wiring is required.
    ///
    /// Capabilities (backends) that need main-thread access receive their
    /// own clone at construction via the closure form of
    /// [`TurRuntimeBuilder::capability`](crate::TurRuntimeBuilder), so they
    /// don't go through this accessor.
    pub fn to_async(&self) -> AsyncPluginContext {
        self.async_cx.clone()
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

// ---------------------------------------------------------------------------
// AsyncPluginContext — the engine's main-thread hop (plugin layer)
// ---------------------------------------------------------------------------

/// `Send + Sync + Clone` handle for posting work onto the engine's **main
/// thread**. The plugin-layer abstraction over the scheduler's raw
/// [`MainTask`](crate::core::scheduler::MainTask) channel.
///
/// The engine creates the channel internally in
/// [`TurRuntimeBuilder::build`](crate::TurRuntimeBuilder) and spawns the
/// paired drain on the main thread, so the hop "just works" with no embedder
/// wiring. Plugins obtain a clone via
/// [`PluginContext::to_async`](PluginContext::to_async); capabilities
/// (backends) that need main-thread access receive their own clone at
/// construction via the closure form of
/// [`TurRuntimeBuilder::capability`](crate::TurRuntimeBuilder).
///
/// Use this to run OS-API calls that require the main thread (e.g. macOS
/// `NSPasteboard` via `arboard` — `flush()` + bridges run on the worker
/// thread after the worker-owns-paint refactor, so any AppKit / Cocoa /
/// Win32 call must hop). The hop is the main-thread analog of a
/// `tokio::runtime::Handle`: a cheap, `Clone + Send + Sync` sender whose
/// paired drain runs received tasks inline + serialized (one `await` per
/// task, in arrival order — safe for non-reentrant OS APIs).
///
/// The result bridge is a reactor-agnostic `oneshot`: the caller polls it on
/// its own thread (typically the worker's executor) and is woken when main
/// completes — no shared executor required.
#[derive(Clone)]
pub struct AsyncPluginContext {
    tx: futures::channel::mpsc::UnboundedSender<crate::core::scheduler::MainTask>,
}

impl AsyncPluginContext {
    /// Wrap a scheduler channel sender. Called once by the engine in
    /// `TurRuntimeBuilder::build` (after creating the channel via
    /// [`scheduler::main_channel`](crate::core::scheduler::main_channel)).
    pub(crate) fn from_sender(
        tx: futures::channel::mpsc::UnboundedSender<crate::core::scheduler::MainTask>,
    ) -> Self {
        Self { tx }
    }

    /// Fire-and-forget: run `fut` on the main thread. Cheap; safe to call
    /// from any thread. The task runs on the main-thread drain (see
    /// [`MainDrain::run`](crate::core::scheduler::MainDrain)); its result is
    /// dropped.
    pub fn spawn_on_main<Fut>(&self, fut: Fut)
    where
        Fut: Future<Output = ()> + Send + 'static,
    {
        let _ = self.tx.unbounded_send(Box::pin(fut));
    }

    /// Run a (synchronous) closure on the main thread and await its result.
    /// Returns a future that resolves to `Ok(output)` once the main thread
    /// has run the closure, or `Err(SpawnError::Dropped)` if the drain was
    /// dropped before it could run (engine shutting down).
    ///
    /// This is the right primitive for OS calls that touch `!Send` platform
    /// handles (e.g. macOS `NSPasteboard`): the closure is constructed on the
    /// worker but **executed** on main, so it may construct + use + drop
    /// `!Send` OS objects entirely on the main thread — they never appear in
    /// a `Send`-checked future's state. Only the closure's captures (which
    /// must be `Send`) and the result `R` cross the thread boundary.
    pub fn run_on_main<R>(&self, f: impl FnOnce() -> R + Send + 'static) -> MainRunFuture<R>
    where
        R: Send + 'static,
    {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.spawn_on_main(async move {
            let r = f();
            let _ = tx.send(r);
        });
        MainRunFuture { rx }
    }

    /// Run an async-producing closure on the main thread and await its
    /// result: the closure `f` is **called on main** (producing the future
    /// there), the future is driven to completion on main, and the result
    /// is bridged back via oneshot.
    ///
    /// The closure must be `Send` (it crosses worker→main); the produced
    /// future `Fut` must be `Send` too (its state is held in the posted
    /// task, which crosses the boundary). Use [`run_on_main`](Self::run_on_main)
    /// instead when the work is synchronous — it imposes no `Send` bound on
    /// the OS objects touched.
    pub fn run_on_main_async<F, Fut, R>(&self, f: F) -> MainRunFuture<R>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.spawn_on_main(async move {
            let r = f().await;
            let _ = tx.send(r);
        });
        MainRunFuture { rx }
    }
}

/// Future returned by [`AsyncPluginContext::run_on_main`] /
/// [`AsyncPluginContext::run_on_main_async`]. Resolves to `Ok(R)` on
/// completion, or `Err(SpawnError::Dropped)` if the drain was dropped
/// (engine shutdown) before running the work.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct MainRunFuture<R> {
    rx: futures::channel::oneshot::Receiver<R>,
}

impl<R> Future for MainRunFuture<R> {
    type Output = Result<R, crate::core::scheduler::SpawnError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.rx).poll(cx) {
            Poll::Ready(Ok(r)) => Poll::Ready(Ok(r)),
            // Canceled ⇒ drain dropped without running the work.
            Poll::Ready(Err(_)) => Poll::Ready(Err(crate::core::scheduler::SpawnError::Dropped)),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::scheduler::{SpawnError, main_channel};
    use std::cell::Cell;
    use std::rc::Rc;

    /// `run_on_main` posts the closure to the drain, the drain runs it, and
    /// the result round-trips back via the oneshot. Drives the caller +
    /// drain together via `join` on one thread; when the caller ends its
    /// handle clone drops, closing the channel so the drain ends and `join`
    /// resolves.
    #[test]
    fn async_context_run_on_main_round_trips() {
        use futures::executor::block_on;
        use futures::future::join;

        let (tx, drain) = main_channel();
        let handle = AsyncPluginContext::from_sender(tx);
        let got = Rc::new(Cell::new(None));
        let got_for_task = got.clone();

        block_on(join(
            async move {
                let v: Result<u32, SpawnError> = handle.run_on_main(|| 7 * 6).await;
                got_for_task.set(v.ok());
            },
            drain.run(),
        ));
        assert_eq!(got.get(), Some(42));
    }

    /// `spawn_on_main` (fire-and-forget) also runs on the drain. The task is
    /// enqueued before the caller ends and drops the handle (the last
    /// sender), so the drain processes the queued task then observes the
    /// closed channel and exits. The task must be `Send` (it crosses into
    /// the drain), so it captures an `Arc<AtomicBool>`, not `Rc`.
    #[test]
    fn async_context_spawn_on_main_runs_on_drain() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        use futures::executor::block_on;
        use futures::future::join;

        let (tx, drain) = main_channel();
        let handle = AsyncPluginContext::from_sender(tx);
        let fired = Arc::new(AtomicBool::new(false));
        let fired_for_task = fired.clone();

        block_on(join(
            async move {
                handle.spawn_on_main(async move {
                    fired_for_task.store(true, Ordering::SeqCst);
                });
            },
            drain.run(),
        ));
        assert!(fired.load(Ordering::SeqCst));
    }
}
