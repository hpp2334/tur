use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::Context;

use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::edgy::reactive::{ReactiveReadStore, SubscriberId};
use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementObject, FragmentHost, NodeTree, NodeTreeData};
use crate::core::layout::SubscribeCx;
use crate::core::view::ViewCx;

// ---------------------------------------------------------------------------
// LayoutViewCx — a `ViewCx` impl backed by a direct `&mut NodeTreeData`
// borrow (the layout phase's exclusive borrow), plus the shared handles.
//
// This is what lets `View::build` run *during layout* (e.g. LazyList mounting
// newly-visible items from inside `perform_layout`). It mutates the same
// `NodeTreeData` the layout pass already holds — no competing `Rc<RefCell>`
// borrow — so build-during-layout is borrow-safe.
//
// The JS realm rides the context (`realm()`, `None` on realm-free instances).
//
// `node_tree` / `mutation_queue` / `dirty` are cloned handles so controllers
// captured at build time (e.g. a ScrollView item) keep working at event time.
// ---------------------------------------------------------------------------

pub struct LayoutViewCx<'a, 'b> {
    tree: &'a mut NodeTreeData,
    /// The JS realm (layout-phase item builders — LazyList/Grid remount).
    boa: Option<&'b mut Context>,
    node_tree: NodeTree,
    mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    dirty: Rc<Cell<bool>>,
}

impl<'a, 'b> LayoutViewCx<'a, 'b> {
    #[allow(clippy::too_many_arguments, dead_code)]
    pub fn new(
        tree: &'a mut NodeTreeData,
        boa: Option<&'b mut Context>,
        node_tree: NodeTree,
        mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
        dirty: Rc<Cell<bool>>,
    ) -> Self {
        LayoutViewCx {
            tree,
            boa,
            node_tree,
            mutation_queue,
            dirty,
        }
    }
}

impl ViewCx for LayoutViewCx<'_, '_> {
    fn alloc_node(&mut self) -> NodeId {
        self.tree.alloc_id()
    }

    fn insert_node(&mut self, id: ElementNodeId, element: AnyElement) {
        let node = ElementObject::new(id, element);
        self.tree.insert_element(node);
    }

    fn realm(&mut self) -> Option<&mut Context> {
        self.boa.as_deref_mut()
    }

    fn insert_fragment(&mut self, host: FragmentHost) {
        self.tree.insert_fragment(host);
    }

    fn link_child(&mut self, parent: NodeId, child: NodeId) {
        self.tree.append_child(parent, child);
        self.tree.mark_dirty(parent);
        self.dirty.set(true);
    }

    fn link_child_before(
        &mut self,
        parent: ElementNodeId,
        child: NodeId,
        ref_child: ElementNodeId,
    ) {
        self.tree.insert_before(parent, child, ref_child);
        self.tree.mark_dirty(parent.into());
        self.dirty.set(true);
    }

    fn move_child_before(&mut self, parent: ElementNodeId, child: NodeId, ref_child: NodeId) {
        self.tree.move_child_before(parent, child, ref_child);
        self.dirty.set(true);
    }

    fn destroy_child(&mut self, id: NodeId) {
        self.tree.destroy_child(id);
        self.dirty.set(true);
    }

    fn mark_dirty(&mut self, id: NodeId) {
        self.tree.mark_dirty(id);
        self.dirty.set(true);
    }

    fn set_query_key(&mut self, id: ElementNodeId, keys: Vec<String>) {
        if let Some(node) = self.tree.get_element_mut(id) {
            node.query_key = if keys.is_empty() { None } else { Some(keys) };
        }
        self.tree.mark_dirty(id.into());
        self.dirty.set(true);
    }

    fn computed_layout(&self, id: ElementNodeId) -> Option<crate::core::layout::ComputedLayout> {
        self.tree.elements.get(&id).map(|n| n.computed_layout)
    }

    fn store_read_only(&self) -> ReactiveReadStore {
        self.tree.read_face.clone()
    }

    fn subscribe_fragment(&self, id: FragmentNodeId) -> SubscribeCx {
        let sub_index = self.tree.store.subscriber_index();
        SubscribeCx::new(sub_index, SubscriberId::new(id.into()))
    }

    fn node_tree(&self) -> NodeTree {
        self.node_tree.clone()
    }

    fn mutation_queue(&self) -> Rc<RefCell<PendingMutationInvocationQueue>> {
        self.mutation_queue.clone()
    }

    fn dirty(&self) -> Rc<Cell<bool>> {
        self.dirty.clone()
    }
}
