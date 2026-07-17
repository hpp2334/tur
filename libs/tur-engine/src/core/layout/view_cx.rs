use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::Context;

use crate::core::mutation::PendingMutationInvocationQueue;
use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementObject, FragmentHost, NodeTree, NodeTreeData};
use crate::core::layout::SubscribeCx;
use crate::core::reactive::{ReactiveReadStore, SubscriberId};
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
// `node_tree` / `mutation_queue` / `dirty` are cloned handles so controllers
// captured at build time (e.g. a ScrollView item) keep working at event time.
// ---------------------------------------------------------------------------

pub struct LayoutViewCx<'a> {
    tree: &'a mut NodeTreeData,
    node_tree: NodeTree,
    mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    dirty: Rc<Cell<bool>>,
}

impl<'a> LayoutViewCx<'a> {
    #[allow(clippy::too_many_arguments, dead_code)]
    pub fn new(
        tree: &'a mut NodeTreeData,
        node_tree: NodeTree,
        mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
        dirty: Rc<Cell<bool>>,
    ) -> Self {
        LayoutViewCx {
            tree,
            node_tree,
            mutation_queue,
            dirty,
        }
    }
}

impl<'a> ViewCx for LayoutViewCx<'a> {
    fn alloc_node(&mut self) -> NodeId {
        self.tree.alloc_id()
    }

    fn insert_node(&mut self, id: ElementNodeId, element: AnyElement, boa: &mut Context) {
        let node = ElementObject::new(id, element, boa);
        self.tree.insert_element(node);
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

    fn computed_layout(&self, id: ElementNodeId) -> Option<tur_shared::ComputedLayout> {
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
