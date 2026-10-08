use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context as TaskContext, Poll};

use crate::core::app::TurAppContext;
use crate::core::capability::{Capabilities, CapabilityDecls};
use crate::core::clock::Clock;
use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::fonts::FontContext;
use crate::core::instance::InstanceContext;
use crate::core::subsystem::Subsystem;
use crate::error::TurError;

/// A plugin that extends the engine with subsystems, rut pkg rows, and/or
/// platform capabilities.
///
/// A plugin is registered on the [`TurRuntime`](crate::TurRuntime) once (via
/// [`TurRuntimeBuilder::plugin`](crate::TurRuntimeBuilder::plugin)). The runtime
/// then drives the plugin through two phases:
///
/// 1. [`compile`](Plugin::compile) — called **once** on the runtime after
///    capabilities are inserted and `requires` is validated. Use it for any
///    one-time, instance-independent work (descriptor caching, validation).
///    Defaults to a no-op.
///
/// 2. [`register`](Plugin::register) — called **once per instance** (per
///    [`TurRuntime::app_builder`](crate::TurRuntime::app_builder) +
///    [`TurAppBuilder::build`](crate::core::runtime::TurAppBuilder::build)).
///    Because `register` takes `&self`, the **same** plugin object is reused
///    across every instance — no factory needed. Stateful per-instance
///    artifacts (subsystems, handles) are created fresh inside `register`
///    and pushed into the per-instance [`PluginRegisterContext`].
///
/// Plugins declare hard-required capabilities via
/// [`requires`](Plugin::requires); the runtime validates every declaration
/// against the registered capabilities before any plugin's `compile`/`register`
/// runs, so a missing capability fails fast at runtime build with a clear error
/// (naming the missing type and the fix) instead of midway through
/// side-effecting registration.
/// `Send + Sync` is required so plugin config can be shared across
/// worker threads (the runtime hands the plugin vec to whichever worker
/// spawns the instance).
pub trait Plugin: Send + Sync {
    /// Declare capabilities this plugin hard-requires. Called by the runtime
    /// builder BEFORE any plugin's `compile`/`register` runs. If a declared
    /// capability is missing, runtime `build()` returns `TurError::Other(...)`
    /// naming the missing type.
    ///
    /// Default: no requirements. Optional capabilities should NOT be declared
    /// here — the plugin should look them up via
    /// [`PluginRegisterContext::capability`] in `register` and handle absence
    /// gracefully.
    fn requires(&self, _decls: &mut CapabilityDecls) {}

    /// One-time, runtime-level compilation. Called once after capabilities are
    /// inserted and `requires` is validated, before any instance is created.
    /// Use it for caching / validation that is identical across every
    /// instance. Defaults to a no-op.
    fn compile(&self, _cx: &mut CompileContext) -> Result<(), TurError> {
        Ok(())
    }

    /// Per-instance registration. Called once per
    /// [`TurAppBuilder::build`](crate::core::runtime::TurAppBuilder::build).
    /// Register subsystems, plugin state, and rut pkg rows via
    /// `ctx.register_subsystem` / `ctx.define_plugin_state` /
    /// `ctx.push_rut_ext`.
    fn register(&self, ctx: &mut PluginRegisterContext) -> Result<(), TurError>;
}

/// Context passed to [`Plugin::compile`]. Provides read access to the
/// runtime-level shared resources: the capability registry (for ad-hoc
/// validation beyond `requires`) and the shared font context.
pub struct CompileContext<'a> {
    pub capabilities: &'a Capabilities,
    pub font_context: &'a FontContext,
}

/// Register-phase context passed to [`Plugin::register`]. It exists **only**
/// while an instance is being constructed — inside
/// [`build_worker_backend`](crate::core::runtime) — and is consumed once the
/// last plugin registers. After the app is built, no further registration is
/// possible: this type is the only registration surface in existence, and it
/// is gone.
///
/// Registration primitives: subsystems, per-instance plugin state, and rut
/// pkg rows (the `tur_host` pkg extension seam). The consumed
/// register-phase collectors are handed to the instance by the builder once
/// the last plugin has registered (see
/// [`PluginRegisterContext::into_parts`]): the flush-subsystem list and the
/// plugin-state map. Installed once; no handle to either collector survives
/// the builder.
pub(crate) struct RegisterParts {
    pub(crate) subsystems: Vec<Box<dyn Subsystem>>,
    pub(crate) plugin_state: HashMap<TypeId, Rc<dyn Any>>,
}

pub struct PluginRegisterContext {
    pub(crate) js_ctx: InstanceContext,
    pub(crate) app: Rc<RefCell<TurAppContext>>,
    /// Build-time collector for plugin-registered flush subsystems. Owned by
    /// this register-phase context and moved into the instance
    /// ([`TurAppInternal::subsystems`](crate::core::app::TurAppInternal))
    /// by the builder after the last plugin registers — see
    /// [`into_parts`](Self::into_parts). No handle into the live
    /// registry survives the builder, so subsystem registration after build
    /// is structurally impossible.
    pub(crate) subsystems: Vec<Box<dyn Subsystem>>,
    /// Build-time collector for plugin-state slots — the subsystems
    /// pattern's twin. Owned by this register-phase context
    /// ([`define_plugin_state`](Self::define_plugin_state) fills it) and
    /// moved into the instance
    /// ([`InstanceContext::install_plugin_state`]) by the builder after
    /// the last plugin registers. No runtime write path exists at all: the
    /// collector is consumed, not flagged.
    pub(crate) plugin_state: HashMap<TypeId, Rc<dyn Any>>,
    /// The engine's [`HostExecutor`] — a `Send + Sync + Clone`
    /// handle for hopping work onto the engine's host thread (for OS APIs
    /// that require it, e.g. macOS `NSPasteboard` via `arboard`). Set by
    /// the engine when the `PluginRegisterContext` is constructed; plugins
    /// obtain a clone via
    /// [`to_host_executor`](PluginRegisterContext::to_host_executor).
    /// Capabilities that need host-thread access receive their own clone at
    /// construction via [`TurRuntimeBuilder::capability`](crate::TurRuntimeBuilder)'s
    /// closure form.
    pub(crate) host_exec: HostExecutor,
}

impl PluginRegisterContext {
    /// Access the shared instance context (reactive store, node tree, etc.).
    pub fn js_ctx(&self) -> &InstanceContext {
        &self.js_ctx
    }

    /// Cheaply-cloned view over the capability registry. Plugins call
    /// `ctx.capability().of::<C>()` to look up sibling capabilities (or
    /// `require::<C>()` for a hard error). Returns a fresh `Capabilities`
    /// handle (single `Rc` bump).
    pub fn capability(&self) -> Capabilities {
        self.js_ctx.capability()
    }

    /// Spawn a worker-side async task, handing it an
    /// [`AsyncWorkerContext`](crate::core::async_::AsyncWorkerContext) for
    /// timers / nested spawns / paint signals. Plugins' async rows use this
    /// instead of the raw scheduler. See
    /// [`InstanceContext::spawn_local`](crate::core::instance::InstanceContext::spawn_local).
    pub fn spawn_local<F, Fut>(&self, f: F) -> crate::core::scheduler::TaskHandle
    where
        F: FnOnce(crate::core::async_::AsyncWorkerContext) -> Fut,
        Fut: std::future::Future<Output = ()> + 'static,
    {
        self.js_ctx.spawn_local(f)
    }

    /// Obtain the engine's [`HostExecutor`] — a `Send + Sync + Clone`
    /// handle for hopping work onto the engine's host thread. Plugins /
    /// subsystems that need to run OS-API calls on the host thread (e.g.
    /// macOS `NSPasteboard` via `arboard`) clone this and call
    /// [`HostExecutor::run_on_host`] (sync closure, result bridged via
    /// oneshot) or [`HostExecutor::spawn_on_host`] (fire-and-forget).
    ///
    /// The hop runs on a serialized drain on the engine's host thread
    /// (safe for non-reentrant OS APIs). The engine creates the channel
    /// internally at `build()` — no embedder wiring is required.
    pub fn to_host_executor(&self) -> HostExecutor {
        self.host_exec.clone()
    }

    /// The engine-wide mutation queue (shared with `flush_pending_mutations`).
    /// Plugins that defer callbacks (e.g. animation `onTick`/`onEnd`) stash
    /// this handle at registration time and push onto the queue when their
    /// subsystem ticks.
    pub fn mutation_queue(&self) -> Rc<RefCell<PendingMutationInvocationQueue>> {
        self.js_ctx.mutation_queue.clone()
    }

    /// The reactive atom minter + writer face. Plugins mint atoms from Rust
    /// via this face (`decl_source` takes a seed value; `build_derive` /
    /// `build_mutate` take Rust closures) and bind them into views / rut
    /// rows. The closures receive typed faces directly — the
    /// auto-dependency tracker works identically.
    ///
    /// Rust-minted atoms materialize per store like any atom. A plugin that
    /// publishes engine environment truth should follow the
    /// `viewportSize$` pattern — the backing's single value home is the
    /// ENGINE store, exposed via a handle derive that reads it through a
    /// captured engine-store read face (so every store of the instance
    /// resolves the same live value), published via the ordinary
    /// `set_source` write rail from a subsystem:
    ///
    /// ```text
    /// let bridge = ctx.reactive();
    /// let backing: Source<Value> = bridge.decl_source(initial);
    /// let engine_read = bridge.read_only();
    /// let handle = bridge.build_derive(move |_read| {
    ///     Ok(engine_read.read(Readable::from(backing)))
    /// });
    /// // in the subsystem tick (no tree chase — works pre- and post-mount):
    /// //   bridge.set_source(backing, value)
    /// ```
    pub fn reactive(&self) -> crate::core::edgy::reactive::ReactiveBridgeStore {
        self.js_ctx.reactive()
    }

    /// The engine's shared clock. Plugins that own time-driven subsystems
    /// (animation, fling inertia, …) stash this handle at registration time
    /// and query `clock.now_millis()` during their tick.
    pub fn clock(&self) -> Rc<dyn Clock> {
        self.app.borrow().frame_env.clock()
    }

    /// The instance context — for plugin rails that hang per-instance
    /// state off it.
    pub fn instance(&self) -> &InstanceContext {
        &self.js_ctx
    }

    /// The build-time viewport (logical CSS pixels) — the size
    /// [`Screen`](crate::core::screen::Screen) carries when plugins
    /// register (no shell `Resize` can have arrived yet). `TurStdPlugin`
    /// seeds the `viewportSize$` atom and the dedup guard of its
    /// [`ResizeSubsystem`](crate::core::screen::ResizeSubsystem) with it.
    pub fn viewport(&self) -> (f64, f64) {
        self.app.borrow().screen.logical_size
    }

    /// Register a [`Subsystem`] — a long-lived participant in the engine's
    /// per-frame `flush` loop. Both flush phases run every fixed-point
    /// iteration, in registration order (= plugin order on
    /// [`TurRuntimeBuilder`](crate::TurRuntimeBuilder)); time-driven
    /// subsystems self-gate via `frame_id`. See the
    /// [`subsystem`](crate::core::subsystem) module docs for details.
    ///
    /// Only available during `register` — the collected list is frozen into
    /// the instance when the last plugin registers.
    pub fn register_subsystem(&mut self, sub: Box<dyn Subsystem>) {
        self.subsystems.push(sub);
    }

    /// Push a rut pkg extension — plugin-owned rows for the `tur_host` pkg
    /// (decl rows at compile time, bodies at boot). See
    /// [`crate::core::rut_runtime::RutPkgExt`].
    pub fn push_rut_ext(&self, ext: crate::core::rut_runtime::RutPkgExt) {
        self.js_ctx.rut_pkg_exts.borrow_mut().push(ext);
    }

    /// Define a per-instance **plugin state** slot — typed state the plugin
    /// owns, readable at runtime via [`InstanceContext::plugin_state`].
    ///
    /// Register-phase only — the collector is consumed by the builder
    /// ([`into_parts`](Self::into_parts)) once the last plugin registers
    /// and installed as an immutable map, so there is no runtime write
    /// path. A duplicate define for the same type panics (fail-fast).
    pub fn define_plugin_state<T: 'static>(&mut self, value: Rc<T>) {
        let id = TypeId::of::<T>();
        if self.plugin_state.contains_key(&id) {
            panic!(
                "plugin_state: `{}` defined twice — each plugin-state type may \
                 be defined only once per instance",
                std::any::type_name::<T>()
            );
        }
        self.plugin_state.insert(id, value);
    }

    /// Builder-facing: consume the register-phase collectors (subsystems +
    /// plugin state), ending the register phase. After this call the only
    /// registration path in existence is gone — the collected state is
    /// installed once and immutable for the instance's lifetime.
    pub(crate) fn into_parts(self) -> RegisterParts {
        RegisterParts {
            subsystems: self.subsystems,
            plugin_state: self.plugin_state,
        }
    }
}

// ---------------------------------------------------------------------------
// HostExecutor — the engine's host-thread hop (plugin layer)
// ---------------------------------------------------------------------------

/// `Send + Sync + Clone` handle for posting work onto the engine's **host
/// thread** (the platform main thread). The plugin-layer abstraction over the scheduler's raw
/// [`HostTask`](crate::core::scheduler::HostTask) channel.
///
/// The engine creates the channel internally in
/// [`TurRuntimeBuilder::build`](crate::TurRuntimeBuilder) and spawns the
/// paired drain on the host thread, so the hop "just works" with no embedder
/// wiring. Plugins obtain a clone via
/// [`PluginRegisterContext::to_host_executor`](PluginRegisterContext::to_host_executor); capabilities
/// (backends) that need host-thread access receive their own clone at
/// construction via the closure form of
/// [`TurRuntimeBuilder::capability`](crate::TurRuntimeBuilder).
///
/// Use this to run OS-API calls that require the host thread (e.g. macOS
/// `NSPasteboard` via `arboard` — `flush()` + bridges run on the worker
/// thread after the worker-owns-paint refactor, so any AppKit / Cocoa /
/// Win32 call must hop). The hop is the host-thread analog of a
/// `tokio::runtime::Handle`: a cheap, `Clone + Send + Sync` sender whose
/// paired drain runs received tasks inline + serialized (one `await` per
/// task, in arrival order — safe for non-reentrant OS APIs).
///
/// The result bridge is a reactor-agnostic `oneshot`: the caller polls it on
/// its own thread (typically the worker's executor) and is woken when the host thread
/// completes — no shared executor required.
#[derive(Clone)]
pub struct HostExecutor {
    tx: futures::channel::mpsc::UnboundedSender<crate::core::scheduler::HostTask>,
}

impl HostExecutor {
    /// Wrap a scheduler channel sender. Called once by the engine in
    /// `TurRuntimeBuilder::build` (after creating the channel via
    /// [`scheduler::host_channel`](crate::core::scheduler::host_channel)).
    pub(crate) fn from_sender(
        tx: futures::channel::mpsc::UnboundedSender<crate::core::scheduler::HostTask>,
    ) -> Self {
        Self { tx }
    }

    /// Fire-and-forget: run `fut` on the host thread. Cheap; safe to call
    /// from any thread. The task runs on the host-thread drain (see
    /// [`HostDrain::run`](crate::core::scheduler::HostDrain)); its result is
    /// dropped.
    pub fn spawn_on_host<Fut>(&self, fut: Fut)
    where
        Fut: Future<Output = ()> + Send + 'static,
    {
        let _ = self.tx.unbounded_send(Box::pin(fut));
    }

    /// Run a (synchronous) closure on the host thread and await its result.
    /// Returns a future that resolves to `Ok(output)` once the host thread
    /// has run the closure, or `Err(SpawnError::Dropped)` if the drain was
    /// dropped before it could run (engine shutting down).
    ///
    /// This is the right primitive for OS calls that touch `!Send` platform
    /// handles (e.g. macOS `NSPasteboard`): the closure is constructed on the
    /// worker but **executed** on the host thread, so it may construct + use + drop
    /// `!Send` OS objects entirely on the host thread — they never appear in
    /// a `Send`-checked future's state. Only the closure's captures (which
    /// must be `Send`) and the result `R` cross the thread boundary.
    pub fn run_on_host<R>(&self, f: impl FnOnce() -> R + Send + 'static) -> HostRunFuture<R>
    where
        R: Send + 'static,
    {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.spawn_on_host(async move {
            let r = f();
            let _ = tx.send(r);
        });
        HostRunFuture { rx }
    }

    /// Run an async-producing closure on the host thread and await its
    /// result: the closure `f` is **called on main** (producing the future
    /// there), the future is driven to completion on main, and the result
    /// is bridged back via oneshot.
    ///
    /// The closure must be `Send` (it crosses worker→host); the produced
    /// future `Fut` must be `Send` too (x    /// task, which crosses the boundary). Use [`run_on_host`](Self::run_on_host)
    /// instead when the work is synchronous — it imposes no `Send` bound on
    /// the OS objects touched.
    pub fn run_on_host_async<F, Fut, R>(&self, f: F) -> HostRunFuture<R>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.spawn_on_host(async move {
            let r = f().await;
            let _ = tx.send(r);
        });
        HostRunFuture { rx }
    }
}

/// Future returned by [`HostExecutor::run_on_host`] /
/// [`HostExecutor::run_on_host_async`]. Resolves to `Ok(R)` on
/// completion, or `Err(SpawnError::Dropped)` if the drain was dropped
/// (engine shutdown) before running the work.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct HostRunFuture<R> {
    rx: futures::channel::oneshot::Receiver<R>,
}

impl<R> Future for HostRunFuture<R> {
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
    use crate::core::scheduler::{SpawnError, host_channel};
    use std::cell::Cell;
    use std::rc::Rc;

    /// `run_on_host` posts the closure to the drain, the drain runs it, and
    /// the result round-trips back via the oneshot. Drives the caller +
    /// drain together via `join` on one thread; when the caller ends its
    /// handle clone drops, closing the channel so the drain ends and `join`
    /// resolves.
    #[test]
    fn host_executor_run_on_host_round_trips() {
        use futures::executor::block_on;
        use futures::future::join;

        let (tx, drain) = host_channel();
        let handle = HostExecutor::from_sender(tx);
        let got = Rc::new(Cell::new(None));
        let got_for_task = got.clone();

        block_on(join(
            async move {
                let v: Result<u32, SpawnError> = handle.run_on_host(|| 7 * 6).await;
                got_for_task.set(v.ok());
            },
            drain.run(),
        ));
        assert_eq!(got.get(), Some(42));
    }

    /// `spawn_on_host` (fire-and-forget) also runs on the drain. The task is
    /// enqueued before the caller ends and drops the handle (the last
    /// sender), so the drain processes the queued task then observes the
    /// closed channel and exits. The task must be `Send` (it crosses into
    /// the drain), so it captures an `Arc<AtomicBool>`, not `Rc`.
    #[test]
    fn host_executor_spawn_on_host_runs_on_drain() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        use futures::executor::block_on;
        use futures::future::join;

        let (tx, drain) = host_channel();
        let handle = HostExecutor::from_sender(tx);
        let fired = Arc::new(AtomicBool::new(false));
        let fired_for_task = fired.clone();

        block_on(join(
            async move {
                handle.spawn_on_host(async move {
                    fired_for_task.store(true, Ordering::SeqCst);
                });
            },
            drain.run(),
        ));
        assert!(fired.load(Ordering::SeqCst));
    }
}
