use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_gc::{Finalize, Trace};
use boa_engine::JsData;

use crate::core::animation::AnimationManager;
use crate::core::edgy_event::PendingMutationInvocationQueue;
use crate::core::elements::ElementTree;
use crate::core::focus::FocusManager;
use crate::core::reactive::Store;
use crate::core::resource::ResourceMap;

#[derive(Clone, Debug, Trace, Finalize, JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TurJsContext {
    pub(crate) element_tree: Rc<RefCell<ElementTree>>,
    pub(crate) mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub(crate) focus_manager: Rc<RefCell<FocusManager>>,
    pub(crate) dirty: Rc<Cell<bool>>,
    pub(crate) resource_map: Rc<RefCell<ResourceMap>>,
    pub(crate) animation_manager: Rc<RefCell<AnimationManager>>,
    pub(crate) store: Store,
}

impl TurJsContext {
    pub fn new(
        element_tree: Rc<RefCell<ElementTree>>,
        mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
        focus_manager: Rc<RefCell<FocusManager>>,
        dirty: Rc<Cell<bool>>,
        resource_map: Rc<RefCell<ResourceMap>>,
        store: Store,
    ) -> Self {
        Self {
            element_tree,
            mutation_queue,
            focus_manager,
            dirty,
            resource_map,
            animation_manager: Rc::new(RefCell::new(AnimationManager::new())),
            store,
        }
    }
}
