use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_gc::{Finalize, Trace};
use boa_engine::JsData;

use crate::core::animation::AnimationManager;
use crate::core::async_::AsyncExecutor;
use crate::core::capability::Capabilities;
use crate::core::mutation::PendingMutationInvocationQueue;
use crate::core::elements::NodeTree;
use crate::core::focus::FocusManager;
use crate::core::reactive::Store;
use crate::core::resource::ResourceMap;

#[derive(Clone, Debug, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurJsContext {
    pub element_tree: NodeTree,
    pub mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub focus_manager: Rc<RefCell<FocusManager>>,
    pub dirty: Rc<Cell<bool>>,
    pub need_paint: Rc<Cell<bool>>,
    pub(crate) resource_map: Rc<RefCell<ResourceMap>>,
    pub animation_manager: Rc<RefCell<AnimationManager>>,
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
        resource_map: Rc<RefCell<ResourceMap>>,
        store: Store,
        async_executor: Rc<AsyncExecutor>,
    ) -> Self {
        Self {
            element_tree,
            mutation_queue,
            focus_manager,
            dirty,
            need_paint,
            resource_map,
            animation_manager: Rc::new(RefCell::new(AnimationManager::new())),
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
    /// this via [`crate::core::bridge::helpers::extract_ctx`] and call
    /// `of::<C>()` / `require::<C>()` to look up backends at JS call time.
    pub fn capability(&self) -> Capabilities {
        self.capabilities.clone()
    }
}
