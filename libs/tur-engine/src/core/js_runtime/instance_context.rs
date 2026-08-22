use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use boa_engine::JsData;
use boa_gc::{Finalize, Trace};

use crate::core::app::HostTx;
use crate::core::async_::CompletionHandle;
use crate::core::capability::Capabilities;
use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::edgy::reactive::Store;
use crate::core::elements::NodeTree;
use crate::core::focus::FocusManager;
use crate::core::image_resource::{ImageManager, ImageResourceId};
use crate::core::scheduler::WorkerContext;

#[derive(Clone, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurInstanceContext {
    /// The instance-owned element tree — created at build, born-bound to the
    /// instance store (the engine-created store handed to the module's
    /// `start({ store })`). `mount(store, view)` with an explicit store
    /// swaps the binding; module teardown clears the root, not the tree.
    pub element_tree: NodeTree,
    pub mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub focus_manager: Rc<RefCell<FocusManager>>,
    pub(crate) dirty: Rc<Cell<bool>>,
    pub(crate) need_paint: Rc<Cell<bool>>,
    /// True while `flush()` is running. Gates [`Self::wake_if_idle`]: paint
    /// requests / dirty marks raised *during* a flush must NOT self-wake
    /// (the flush is already driving this frame); only out-of-flush raises
    /// (module eval, a spawned task's tick, a reactive `set` from a callback)
    /// emit a `WorkerMsg::Wake` to re-arm an idle worker.
    pub(crate) in_flush: Rc<Cell<bool>>,
    /// Coalescing gate for self-wakes: at most one `WorkerMsg::Wake` is
    /// outstanding at a time. Set when [`Self::wake_if_idle`] emits, cleared
    /// at `flush()` start (so the next out-of-flush raise can emit again).
    pub(crate) wake_pending: Rc<Cell<bool>>,
    /// Self-wake callback — sends `WorkerMsg::Wake` to this worker's own
    /// loop. Shared with `CompletionQueue` / `FlushTaskQueue`; reused by
    /// [`Self::wake_if_idle`] so any paint-worthy state change outside a
    /// flush re-arms an idle worker without main involvement.
    ///
    /// Sound to keep out of boa's GC trace: pure Rust state
    /// (`Arc<dyn Fn() + Send + Sync>`), no `boa_gc::Gc`/`GcRefCell`. The
    /// struct-level `#[boa_gc(unsafe_empty_trace)]` already covers this same
    /// trade-off for the other fields.
    pub(crate) wake_worker: Arc<dyn Fn() + Send + Sync>,
    /// Worker-side image state: the natural-size map plus the next-id
    /// counter, bundled into one [`ImageManager`] (id allocation + size
    /// recording are atomic). Mutated by [`Self::register_image`] at decode
    /// time; read by layout + paint (`get_image_natural_size` /
    /// `PaintContext::get_image_size`).
    pub(crate) image_manager: Rc<RefCell<ImageManager>>,
    pub(crate) store: Store,
    /// Worker→main channel sender clone. Bridges use it to ship messages
    /// directly to main without a staging vec — most notably
    /// [`Self::register_image`] sends one `HostMsg::UploadImage` per decoded
    /// image. FIFO is preserved across the shared channel (the bridge
    /// enqueues during flush; `worker_loop` enqueues after flush), so main
    /// always uploads an image before playing back the frame that uses it.
    ///
    /// Sound to keep out of boa's GC trace: it's pure Rust state
    /// (`futures::channel::mpsc::UnboundedSender`), no `boa_gc::Gc`/
    /// `GcRefCell`. The struct-level `#[boa_gc(unsafe_empty_trace)]` already
    /// covers this same trade-off for the other fields.
    pub(crate) host_tx: HostTx,
    /// Worker-thread scheduler — bridges call `spawn_local(fut)` to drive
    /// async work (clipboard reads, http requests, sleep futures). Set by
    /// `build_worker_backend` from the worker_ctx passed by the runtime.
    ///
    /// Sound to keep out of boa's GC trace: it's pure Rust state
    /// Worker-thread scheduler view (`WorkerContext`, wrapping an
    /// `Rc<dyn WorkerContextDriver>`), no `boa_gc::Gc`/`GcRefCell`. The
    /// struct-level `#[boa_gc(unsafe_empty_trace)]` already covers this
    /// same trade-off for the other fields.
    pub(crate) worker_ctx: WorkerContext,
    /// Cheap-cloned completion handle — bridges call `push(closure)` from
    /// inside spawned futures to settle JsPromises under `&mut Context` on
    /// the next flush. Pushing fires `on_push`, which self-sends
    /// `WorkerMsg::Wake` so the worker flushes promptly.
    pub(crate) completion_handle: CompletionHandle,
    /// Cheap-cloned handle to the flush-driven task queue — `sleep` +
    /// `launch` push their driver futures here (instead of
    /// `worker_ctx.spawn_local`) so `flush` polls them in lockstep with
    /// completions / microtasks. See `core::async_::flush_tasks`.
    pub(crate) flush_task_handle: crate::core::async_::FlushTaskHandle,
    /// Type-erased capability registry shared with the engine builder,
    /// plugin context, event handlers, and ctx-bound bridge fns. Plugins
    /// declare their hard dependencies via [`Plugin::requires`] so the engine
    /// builder can validate them at `build()` time before any plugin's
    /// `register` runs; lookups are deferred to call/dispatch time via
    /// [`TurInstanceContext::capability`].
    ///
    /// Sound to keep out of boa's GC trace: the registry is pure Rust state
    /// (a `HashMap<TypeId, Box<dyn Any>>` behind an `Rc<RefCell<…>>`), no
    /// `boa_gc::Gc`/`GcRefCell`. The struct-level
    /// `#[boa_gc(unsafe_empty_trace)]` already covers this same trade-off for
    /// the other fields.
    ///
    /// [`Plugin::requires`]: crate::core::plugin::Plugin::requires
    pub capabilities: Capabilities,
    /// Worker-side per-instance metadata — a typed key→value map seeded
    /// at **build time** (via
    /// [`TurAppBuilder::instance_data`](crate::core::runtime::TurAppBuilder::instance_data),
    /// whose closure runs on the worker right before plugin `register`)
    /// and **read / updated** at runtime via [`Self::data`] /
    /// [`Self::with_data`] / [`Self::update`]. Never accessible to JS
    /// itself, so it carries secure, JS-unforgeable identity (e.g. a host
    /// `PluginId` for plugin systems where bridge fns must resolve the
    /// calling plugin without trusting JS arguments).
    ///
    /// Define-vs-update split (strict, fail-fast):
    /// - **Build time** ([`InstanceDataCx::define`]): initial definition
    ///   per `TypeId`; panics on duplicate.
    /// - **Runtime** ([`Self::update`]): replace an existing value;
    ///   panics if the `TypeId` was not defined at build time.
    /// - **Runtime read** ([`Self::data`] / [`Self::with_data`]):
    ///   unchanged; return `None` for unstamped types.
    ///
    /// Mirrors the `capabilities` field's shape and soundness trade-off:
    /// pure Rust state behind an `Rc<RefCell<…>>`, shared across every
    /// cheap clone of `TurInstanceContext` (one per bridge call, per flush, etc.).
    /// The struct-level `#[boa_gc(unsafe_empty_trace)]` already covers it.
    pub instance_data: Rc<RefCell<HashMap<TypeId, Box<dyn Any>>>>,
}

impl TurInstanceContext {
    #[allow(clippy::too_many_arguments)]
    /// `capabilities` is the shared registry owned by the
    /// [`TurRuntime`](crate::TurRuntime) — every instance spawned from one
    /// runtime shares the same capability backends (Clipboard/Http/etc.).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        element_tree: NodeTree,
        mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
        focus_manager: Rc<RefCell<FocusManager>>,
        dirty: Rc<Cell<bool>>,
        need_paint: Rc<Cell<bool>>,
        image_manager: Rc<RefCell<ImageManager>>,
        host_tx: HostTx,
        store: Store,
        worker_ctx: WorkerContext,
        completion_handle: CompletionHandle,
        flush_task_handle: crate::core::async_::FlushTaskHandle,
        wake_worker: Arc<dyn Fn() + Send + Sync>,
        capabilities: Capabilities,
    ) -> Self {
        Self {
            element_tree,
            mutation_queue,
            focus_manager,
            dirty,
            need_paint,
            in_flush: Rc::new(Cell::new(false)),
            wake_pending: Rc::new(Cell::new(false)),
            wake_worker,
            image_manager,
            host_tx,
            store,
            worker_ctx,
            completion_handle,
            flush_task_handle,
            capabilities,
            instance_data: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// Mark this frame as paint-worthy AND, if the worker is idle (not
    /// currently flushing) and no wake is already outstanding, emit a
    /// coalesced `WorkerMsg::Wake` so the worker's own loop pumps a flush.
    /// This is the worker's self-drive mechanism: any paint-worthy state
    /// change raised outside a `flush()` (module eval, a spawned task's
    /// tick, a reactive `set` from a callback) re-arms an idle worker
    /// without main's involvement. During a `flush()` it is a no-op (the
    /// flush is already driving this frame).
    pub fn request_frame(&self) {
        self.need_paint.set(true);
        self.wake_if_idle();
    }

    /// Set the host-dirty flag (tree mutation / reactive change). Flag-only —
    /// the next flush reads it. Out-of-flush render-after-load is covered by
    /// `wake_if_dirty` (post-eval), so this does NOT self-wake (per-mutation
    /// self-wakes during eval would desynchronize pump-based sequencing).
    pub fn set_dirty(&self) {
        self.dirty.set(true);
    }

    /// Emit a coalesced `WorkerMsg::Wake` unless the worker is mid-flush or
    /// one is already outstanding. The outstanding gate (`wake_pending`) is
    /// cleared at `flush()` start.
    pub(crate) fn wake_if_idle(&self) {
        if !self.in_flush.get() && !self.wake_pending.get() {
            self.wake_pending.set(true);
            (self.wake_worker)();
        }
    }

    /// Called by `flush()` at entry: the pump consuming the wake has begun,
    /// so re-arm the coalescing gate for any paint requests raised during
    /// this flush (they must be able to emit a fresh wake for the *next* pump).
    pub(crate) fn begin_flush(&self) {
        self.in_flush.set(true);
        self.wake_pending.set(false);
    }

    /// Called by `flush()` at exit.
    pub(crate) fn end_flush(&self) {
        self.in_flush.set(false);
    }

    /// Spawn a worker-side task, handing it an [`AsyncWorkerContext`] for
    /// timers / nested spawns / paint signals. The raw `WorkerContext` is
    /// not exposed to spawn sites — this is the entry point for async work
    /// that needs engine interaction (e.g. the caret-blink loop). The
    /// closure receives the context by value; capture it (`async move`) into
    /// the returned future to use it across `.await`s.
    ///
    /// ```text
    /// js_ctx.spawn_local(|aw| async move {
    ///     loop {
    ///         aw.sleep(half_period).await;
    ///         aw.request_frame();
    ///     }
    /// });
    /// ```
    pub fn spawn_local<F, Fut>(&self, f: F) -> crate::core::scheduler::TaskHandle
    where
        F: FnOnce(crate::core::async_::AsyncWorkerContext) -> Fut,
        Fut: std::future::Future<Output = ()> + 'static,
    {
        let aw = crate::core::async_::AsyncWorkerContext::new(self.clone());
        let fut = f(aw);
        self.worker_ctx.spawn_local(Box::pin(fut))
    }

    /// Worker-thread scheduler. Core-internal only — engine-internal bridge
    /// fns (`sleep` / `launch`) and [`AsyncWorkerContext`] use it for timers
    /// / nested spawns. External async work goes through [`Self::spawn_local`]
    /// (which hands the task an [`AsyncWorkerContext`]).
    pub(crate) fn worker_ctx(&self) -> &WorkerContext {
        &self.worker_ctx
    }

    /// Cheap-cloned completion handle. Bridge fns extract this via
    /// `extract_js_ctx` and call `push(closure)` from inside spawned futures
    /// to settle JsPromises under `&mut Context` on the next flush.
    pub fn completion_handle(&self) -> CompletionHandle {
        self.completion_handle.clone()
    }

    /// Cheap-cloned flush-task handle. `sleep` + `launch` extract this via
    /// `extract_js_ctx` and call `spawn(fut)` to push engine-internal driver
    /// futures onto the flush-driven queue.
    pub fn flush_task_handle(&self) -> crate::core::async_::FlushTaskHandle {
        self.flush_task_handle.clone()
    }

    /// Cheaply-cloned view over the capability registry. Bridge fns extract
    /// this via [`crate::core::js_runtime::helpers::extract_js_ctx`] and call
    /// `of::<C>()` / `require::<C>()` to look up backends at JS call time.
    pub fn capability(&self) -> Capabilities {
        self.capabilities.clone()
    }

    /// The reactive atom minter + writer face. Bridge fns that need to
    /// mint atoms mid-call (e.g. a hypothetical `useResource()` factory
    /// returning a fresh source) reach this via
    /// [`extract_js_ctx`](crate::core::js_runtime::helpers::extract_js_ctx).
    /// Plugins reach the same face at registration time via
    /// [`PluginContext::reactive`](crate::core::plugin::PluginContext::reactive).
    pub fn reactive(&self) -> crate::core::edgy::reactive::ReactiveBridgeStore {
        self.store.bridge()
    }

    /// Dev-tool introspection over the shared reactive subscriber graph:
    /// `(live subscribers, total declared atom↔subscriber edges)`.
    pub fn reactive_subscriber_stats(&self) -> (usize, usize) {
        self.store.subscriber_stats()
    }

    /// Register a decoded image: allocate the worker-side id + record its
    /// natural size via [`ImageManager::allocate`] (so layout + paint can
    /// size the element this frame), and ship the pixel `Blob` to main
    /// directly via `HostMsg::UploadImage` on the shared `host_tx` channel.
    /// Stays synchronous — `unbounded_send` is non-blocking, and the FIFO
    /// channel guarantees main receives the `UploadImage` before any
    /// `RenderCommands` that plays the image back (the bridge enqueues
    /// during flush; `worker_loop` enqueues after flush).
    pub fn register_image(
        &self,
        image: crate::core::image_resource::ImageResource,
    ) -> ImageResourceId {
        let id = self.image_manager.borrow_mut().allocate(&image);
        let _ = self
            .host_tx
            .unbounded_send(crate::core::app::HostMsg::UploadImage { id, image });
        id
    }

    /// Runtime **update** of an existing per-instance data slot. The slot
    /// must have been defined at build time via
    /// [`TurAppBuilder::instance_data`](crate::core::runtime::TurAppBuilder::instance_data);
    /// calling `update` for a `TypeId` that was never defined panics
    /// (fail-fast — catches missing build-time `define` immediately).
    ///
    /// Plugins reach this via `ctx.js_ctx()` from `register`, bridge fns
    /// via [`extract_js_ctx`](crate::core::js_runtime::helpers::extract_js_ctx).
    /// Use [`InstanceDataCx::define`] at build time for the initial value.
    pub fn update<T: Any + 'static>(&self, value: T) {
        let mut map = self.instance_data.borrow_mut();
        let id = TypeId::of::<T>();
        if !map.contains_key(&id) {
            panic!(
                "instance_data: `{}` was not defined at build time — \
                 add `.instance_data(|cx| cx.define::<{}>(...))` on the TurAppBuilder",
                std::any::type_name::<T>(),
                std::any::type_name::<T>()
            );
        }
        map.insert(id, Box::new(value));
    }

    /// Read a previously-stamped value out of the per-instance data map by
    /// type. Returns a clone of the stored value (mirrors
    /// [`Capabilities::of`]). Use [`Self::with_data`] when `T` is not
    /// `Clone` or to avoid cloning a large value.
    ///
    /// Bridge fns reach this via
    /// [`extract_js_ctx`](crate::core::js_runtime::helpers::extract_js_ctx):
    ///
    /// ```text
    /// fn storage_get(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    ///     let js_ctx = extract_js_ctx(args)?;
    ///     let plugin_id = js_ctx.data::<PluginId>()
    ///         .ok_or_else(|| JsError::from(JsNativeError::typ()
    ///             .with_message("ease: no plugin context bound to this instance")))?;
    ///     // plugin_id.0 is host-guaranteed — never from JS args
    ///     let key = args.get_or_undefined(1).as_string()...;
    ///     ...
    /// }
    /// ```
    pub fn data<T: Any + Clone + 'static>(&self) -> Option<T> {
        self.instance_data
            .borrow()
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }

    /// Read a previously-stamped value via a callback that runs under the
    /// map's borrow, returning whatever the callback produces. The callback
    /// receives `&T`, so `T` need not be `Clone`. Returns `None` if no
    /// value of type `T` is stamped on this instance.
    ///
    /// Use this over [`Self::data`] when `T` is expensive to clone or not
    /// `Clone` at all — e.g. a large config struct or a closure-carrying
    /// handle.
    pub fn with_data<T: Any + 'static, R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        self.instance_data
            .borrow()
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .map(f)
    }
}

/// Build-time context handed to the closure passed to
/// [`TurAppBuilder::instance_data`](crate::core::runtime::TurAppBuilder::instance_data).
/// It exposes **only** [`Self::define`] — the initial value for each typed
/// slot. The closure runs on the worker (right after `TurInstanceContext`
/// is constructed, before any plugin `register`), so values built fresh
/// inside the closure body never cross the main↔worker boundary; only
/// values captured by the closure need to be `Send`.
///
/// This is the **only** way to introduce a new `TypeId` into the per-instance
/// data map. Runtime code (plugins, bridge fns, subsystem flushes) can only
/// [`TurInstanceContext::update`] (replace an existing value) or
/// [`TurInstanceContext::data`] / [`TurInstanceContext::with_data`] (read) —
/// they cannot introduce new types.
///
/// `define` is strict: defining the same `TypeId` twice panics (fail-fast).
pub struct InstanceDataCx {
    data: Rc<RefCell<HashMap<TypeId, Box<dyn Any>>>>,
}

impl InstanceDataCx {
    /// Construct from a clone of the context's `instance_data` handle.
    /// Engine-internal — only `build_worker_backend` calls this when it
    /// replays the definer closure captured by `TurAppBuilder`.
    pub(crate) fn from_map(data: Rc<RefCell<HashMap<TypeId, Box<dyn Any>>>>) -> Self {
        Self { data }
    }

    /// Define (initial value for) a typed slot. Panics if `T` was already
    /// defined — each type may be defined exactly once per instance.
    /// Mirrors the build-time-only contract: runtime code must use
    /// [`TurInstanceContext::update`] (which in turn requires the slot to
    /// have been defined here first).
    pub fn define<T: Any + 'static>(&self, value: T) {
        let mut map = self.data.borrow_mut();
        let id = TypeId::of::<T>();
        if map.contains_key(&id) {
            panic!(
                "instance_data: `{}` already defined — each type may be defined only once \
                 (duplicate `define` in TurAppBuilder::instance_data closure)",
                std::any::type_name::<T>()
            );
        }
        map.insert(id, Box::new(value));
    }
}
