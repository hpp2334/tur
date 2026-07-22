use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_gc::{Finalize, Trace};
use boa_engine::JsData;

use crate::core::async_::AsyncExecutor;
use crate::core::capability::Capabilities;
use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::elements::NodeTree;
use crate::core::focus::FocusManager;
use crate::core::image_resource::ImageResourceMap;
use crate::core::edgy::reactive::Store;

#[derive(Clone, Debug, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurJsContext {
    pub element_tree: NodeTree,
    pub mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub focus_manager: Rc<RefCell<FocusManager>>,
    pub dirty: Rc<Cell<bool>>,
    pub need_paint: Rc<Cell<bool>>,
    pub(crate) image_resource_map: Rc<RefCell<ImageResourceMap>>,
    pub(crate) store: Store,
    /// Engine-owned async executor. Always present (created in
    /// [`crate::core::app::TurAppInternal::new`]); exposed to ctx-bound bridge
    /// fns via [`TurJsContext::async_executor`] instead of the capability
    /// registry, since — unlike `Clipboard`/`Http` — it is not a swappable
    /// plugin backend.
    pub(crate) async_executor: Rc<AsyncExecutor>,
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
    pub fn new(
        element_tree: NodeTree,
        mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
        focus_manager: Rc<RefCell<FocusManager>>,
        dirty: Rc<Cell<bool>>,
        need_paint: Rc<Cell<bool>>,
        image_resource_map: Rc<RefCell<ImageResourceMap>>,
        store: Store,
        async_executor: Rc<AsyncExecutor>,
    ) -> Self {
        Self {
            element_tree,
            mutation_queue,
            focus_manager,
            dirty,
            need_paint,
            image_resource_map,
            store,
            async_executor,
            capabilities: Capabilities::new(),
        }
    }

    /// Engine-owned async executor. Always present.
    pub fn async_executor(&self) -> &Rc<AsyncExecutor> {
        &self.async_executor
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
