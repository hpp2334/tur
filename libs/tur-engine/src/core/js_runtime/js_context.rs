use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::JsData;
use boa_gc::{Finalize, Trace};

use crate::core::app::MainTx;
use crate::core::async_::CompletionHandle;
use crate::core::capability::Capabilities;
use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::edgy::reactive::Store;
use crate::core::elements::NodeTree;
use crate::core::focus::FocusManager;
use crate::core::image_resource::{ImageMetadata, ImageMetadataMap, ImageResourceId};
use crate::core::scheduler::WorkerScheduler;

#[derive(Clone, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurJsContext {
    pub element_tree: NodeTree,
    pub mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub focus_manager: Rc<RefCell<FocusManager>>,
    pub dirty: Rc<Cell<bool>>,
    pub need_paint: Rc<Cell<bool>>,
    /// Worker-side image metadata (natural sizes only — the pixel `Blob`
    /// lives on main). Inserted by [`Self::register_image`] at decode time;
    /// read by layout + paint (`get_image_natural_size` /
    /// `PaintContext::get_image_size`).
    pub(crate) image_metadata_map: Rc<RefCell<ImageMetadataMap>>,
    /// Next worker-assigned image id (worker id authority — main inserts
    /// under these ids via `ImageResourceMap::insert_with_id`).
    pub(crate) image_next_id: Rc<Cell<u64>>,
    pub(crate) store: Store,
    /// Worker→main channel sender clone. Bridges use it to ship messages
    /// directly to main without a staging vec — most notably
    /// [`Self::register_image`] sends one `MainMsg::UploadImage` per decoded
    /// image. FIFO is preserved across the shared channel (the bridge
    /// enqueues during flush; `worker_loop` enqueues after flush), so main
    /// always uploads an image before playing back the frame that uses it.
    ///
    /// Sound to keep out of boa's GC trace: it's pure Rust state
    /// (`futures::channel::mpsc::UnboundedSender`), no `boa_gc::Gc`/
    /// `GcRefCell`. The struct-level `#[boa_gc(unsafe_empty_trace)]` already
    /// covers this same trade-off for the other fields.
    pub(crate) main_tx: MainTx,
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
        image_metadata_map: Rc<RefCell<ImageMetadataMap>>,
        image_next_id: Rc<Cell<u64>>,
        main_tx: MainTx,
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
            image_metadata_map,
            image_next_id,
            main_tx,
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

    /// Worker-side image metadata map. Bridge fns (e.g. `createImageResource`)
    /// register decoded images via [`Self::register_image`]; layout
    /// (`get_image_natural_size`) and paint (`get_image_size`) read it during
    /// the layout/paint passes. Contains sizes only — the pixel `Blob` is
    /// shipped to main directly via [`Self::main_tx`] at decode time.
    pub fn image_metadata_map(&self) -> &Rc<RefCell<ImageMetadataMap>> {
        &self.image_metadata_map
    }

    /// Register a decoded image: assign the worker-side id, record its
    /// natural size in [`Self::image_metadata_map`] (so layout + paint can
    /// size the element this frame), and ship the pixel `Blob` to main
    /// directly via `MainMsg::UploadImage` on the shared `main_tx` channel.
    /// Stays synchronous — `unbounded_send` is non-blocking, and the FIFO
    /// channel guarantees main receives the `UploadImage` before any
    /// `RenderCommands` that plays the image back (the bridge enqueues
    /// during flush; `worker_loop` enqueues after flush).
    pub fn register_image(
        &self,
        image: crate::core::image_resource::ImageResource,
    ) -> ImageResourceId {
        let id = ImageResourceId::new(self.image_next_id.get());
        self.image_next_id.set(id.as_u64() + 1);
        let size = image.natural_size;
        self.image_metadata_map
            .borrow_mut()
            .insert(id, ImageMetadata { size });
        let _ = self
            .main_tx
            .unbounded_send(crate::core::app::MainMsg::UploadImage { id, image });
        id
    }
}
