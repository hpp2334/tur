use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use boa_gc::{Finalize, Trace};
use boa_engine::JsData;

use crate::core::animation::AnimationManager;
use crate::core::async_::AsyncExecutor;
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
    /// registry, since — unlike `Http`/`Clipboard` — it is not a swappable
    /// plugin backend.
    pub(crate) async_executor: Rc<AsyncExecutor>,
    /// Type-erased capability slots for plugin-injected services (e.g.
    /// `Rc<dyn Http>`, `Rc<dyn Clipboard>`, `Rc<AsyncExecutor>`).
    ///
    /// Plugins insert via [`TurJsContext::insert_capability`] during
    /// [`crate::core::plugin::Plugin::register`]; bridge fns read via
    /// [`TurJsContext::capability`]. Stored as `Box<dyn Any>` keyed by
    /// `TypeId::of::<T>()` — the engine doesn't need to know the concrete
    /// capability types.
    ///
    /// Sound to keep out of boa's GC trace: every entry is pure Rust state
    /// (`Rc<dyn Trait>`, no `boa_gc::Gc`/`GcRefCell`), and the registry
    /// itself lives in `Rc<RefCell<...>>` which dies with `TurJsContext`.
    /// The struct-level `#[boa_gc(unsafe_empty_trace)]` already covers this
    /// same trade-off for the other fields.
    pub capabilities: Rc<RefCell<HashMap<TypeId, Box<dyn Any>>>>,
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
            capabilities: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// Engine-owned async executor. Always present.
    pub fn async_executor(&self) -> &Rc<AsyncExecutor> {
        &self.async_executor
    }

    /// Insert a capability under `TypeId::of::<T>()`. Plugins call this from
    /// [`crate::core::plugin::Plugin::register`] to expose their backend to
    /// ctx-bound bridge fns. A second insert for the same type overwrites.
    pub fn insert_capability<T: Any + 'static>(&self, cap: T) {
        self.capabilities
            .borrow_mut()
            .insert(TypeId::of::<T>(), Box::new(cap));
    }

    /// Read a capability by type. Returns a clone of the stored value (so
    /// `T` should be a cheaply-clonable handle like `Rc<dyn Trait>`).
    pub fn capability<T: Any + Clone + 'static>(&self) -> Option<T> {
        self.capabilities
            .borrow()
            .get(&TypeId::of::<T>())
            .and_then(|c| c.downcast_ref::<T>())
            .cloned()
    }
}
