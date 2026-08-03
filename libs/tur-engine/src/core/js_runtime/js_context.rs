use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::JsData;
use boa_gc::{Finalize, Trace};

use crate::core::async_::CompletionHandle;
use crate::core::capability::Capabilities;
use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::edgy::reactive::Store;
use crate::core::elements::NodeTree;
use crate::core::focus::FocusManager;
use crate::core::image_resource::ImageResourceMap;
use crate::core::scheduler::WorkerScheduler;

#[derive(Clone, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurJsContext {
    pub element_tree: NodeTree,
    pub mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub focus_manager: Rc<RefCell<FocusManager>>,
    pub dirty: Rc<Cell<bool>>,
    pub need_paint: Rc<Cell<bool>>,
    pub(crate) image_resource_map: Rc<RefCell<ImageResourceMap>>,
    pub(crate) store: Store,
    /// Worker-thread scheduler — bridges call `spawn_local(fut)` to drive
    /// async work (clipboard reads, http requests, sleep futures). Set by
    /// `build_worker_backend` from the worker_sched passed by the runtime.
    ///
    /// Sound to keep out of boa's GC trace: it's pure Rust state
    /// (`Rc<dyn WorkerScheduler>`), no `boa_gc::Gc`/`GcRefCell`. The
    /// struct-level `#[boa_gc(unsafe_empty_trace)]` already covers this
    /// same trade-off for the other fields.
    pub(crate) worker_sched: Rc<dyn WorkerScheduler>,
    /// Cheap-cloned completion handle — bridges call `push(closure)` from
    /// inside spawned futures to settle JsPromises under `&mut Context` on
    /// the next flush. Pushing fires `on_push`, which self-sends
    /// `WorkerMsg::Wake` so the worker flushes promptly.
    pub(crate) completion_handle: CompletionHandle,
    /// Type-erased capability registry shared with the engine builder,
    /// plugin context, event handlers, and ctx-bound bridge fns. Plugins
    /// declare their hard dependencies via [`Plugin::requires`] so the engine
    /// builder can validate them at `build()` time before any plugin's
    /// `register` runs; lookups are deferred to call/dispatch time via
    /// [`TurJsContext::capability`].
    ///
    /// Sound to keep out of boa's GC trace: the registry is pure Rust state
    /// (a `HashMap<TypeId, Box<dyn Any>>` behind an `Rc<RefCell<…>>`), no
    /// `boa_gc::Gc`/`GcRefCell`. The struct-level
    /// `#[boa_gc(unsafe_empty_trace)]` already covers this same trade-off for
    /// the other fields.
    ///
    /// [`Plugin::requires`]: crate::core::plugin::Plugin::requires
    pub capabilities: Capabilities,
}

impl TurJsContext {
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
        image_resource_map: Rc<RefCell<ImageResourceMap>>,
        store: Store,
        worker_sched: Rc<dyn WorkerScheduler>,
        completion_handle: CompletionHandle,
        capabilities: Capabilities,
    ) -> Self {
        Self {
            element_tree,
            mutation_queue,
            focus_manager,
            dirty,
            need_paint,
            image_resource_map,
            store,
            worker_sched,
            completion_handle,
            capabilities,
        }
    }

    /// Worker-thread scheduler. Bridge fns extract this via
    /// [`crate::core::js_runtime::helpers::extract_ctx`] and call
    /// `spawn_local(fut)` to drive async work.
    pub fn worker_sched(&self) -> &Rc<dyn WorkerScheduler> {
        &self.worker_sched
    }

    /// Cheap-cloned completion handle. Bridge fns extract this via
    /// `extract_ctx` and call `push(closure)` from inside spawned futures
    /// to settle JsPromises under `&mut Context` on the next flush.
    pub fn completion_handle(&self) -> CompletionHandle {
        self.completion_handle.clone()
    }

    /// Cheaply-cloned view over the capability registry. Bridge fns extract
    /// this via [`crate::core::js_runtime::helpers::extract_ctx`] and call
    /// `of::<C>()` / `require::<C>()` to look up backends at JS call time.
    pub fn capability(&self) -> Capabilities {
        self.capabilities.clone()
    }

    /// Image resource map. Bridge fns (e.g. `createImageResource` in
    /// `tur-image`) extract this via `extract_ctx` and call
    /// `borrow_mut().insert_image(...)` to register decoded images. Layout
    /// (`get_image_natural_size`) and the renderers (`iter_images`) read it
    /// during the paint pass.
    pub fn image_resource_map(&self) -> &Rc<RefCell<ImageResourceMap>> {
        &self.image_resource_map
    }
}

