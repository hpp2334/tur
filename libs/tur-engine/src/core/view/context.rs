use boa_engine::{Context, JsValue};

use crate::core::edgy::reactive::{ReactiveReadStore, Readable, Store, SubscriberId};
use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementObject, FragmentHost, NodeTree};
use crate::core::js_runtime::TurInstanceContext;
use crate::core::layout::SubscribeCx;
use crate::core::view::build_cx::controller_handles;
use crate::core::view::{View, ViewCx};

/// Context for building specs into the ElementTree and running effects.
/// Provides scoped access to the tree and the reactive store.
///
/// The boa `Context` is passed alongside (not stored) so callers can reborrow
/// freely while holding `&mut SharedViewCx`.
pub struct SharedViewCx {
    js_ctx: TurInstanceContext,
}

impl SharedViewCx {
    pub fn new(js_ctx: TurInstanceContext) -> Self {
        SharedViewCx { js_ctx }
    }

    pub fn js_ctx(&self) -> &TurInstanceContext {
        &self.js_ctx
    }

    // ----- reactive store -----------------------------------------------------

    /// The **mounted** store — the store the tree is currently bound to (the
    /// instance store from `start({ store })`, or a later explicit
    /// `mount(store, view)` swap). Declarations materialize into this store
    /// from every build-time read/subscribe path.
    pub fn mounted_store(&self) -> Store {
        self.js_ctx.element_tree.store()
    }

    /// Read-only view of the mounted store for resolving atom values.
    pub fn store_read_only(&self) -> ReactiveReadStore {
        self.mounted_store().read_only()
    }

    /// Read an atom's current value as a raw `JsValue` (untracked).
    pub fn read_atom_raw<T>(&self, readable: Readable<T>, boa: &mut Context) -> JsValue {
        self.store_read_only().read(readable, boa)
    }

    /// Create a `SubscribeCx` scoped to a fragment, so the fragment can
    /// declare its reactive atom deps at build time. On drop, the deps are
    /// atomically swapped into the subscriber graph.
    pub fn subscribe_fragment(&self, id: FragmentNodeId) -> SubscribeCx {
        let sub_index = self.mounted_store().subscriber_index();
        SubscribeCx::new(sub_index, SubscriberId::new(id.into()))
    }

    /// Resolve a `Val<T>` to its current `T` value.  For reactive vals the
    /// atom is lazily read from the store (untracked).  Used during the effect
    /// phase; layout uses `LayoutContext::read_val` (with subscriber tracking).
    /// Declaration ids materialize into the mounted store.
    pub fn read_val<T: crate::core::view::FromJs + Clone + 'static>(
        &self,
        val: &crate::core::view::Val<T>,
        boa: &mut Context,
    ) -> Option<T> {
        use crate::core::view::Val;
        match val {
            Val::Static(t) => Some(t.clone()),
            Val::Reactive(readable) => {
                let js = self.store_read_only().read(*readable, boa);
                T::from_js(&js).ok()
            }
        }
    }

    // ----- ElementTree helpers ------------------------------------------------

    /// Allocate a fresh node id.
    pub fn alloc_node(&self) -> NodeId {
        self.js_ctx.element_tree.borrow_mut().alloc_id()
    }

    /// Create an `AnyElement`-backed tree node and insert it (no parent yet).
    pub fn insert_node(&self, id: ElementNodeId, element: AnyElement, boa: &mut Context) {
        let node = ElementObject::new(id, element, boa);
        self.js_ctx.element_tree.borrow_mut().insert_element(node);
    }

    /// Insert a `FragmentHost` into the fragments map.
    pub fn insert_fragment(&self, host: FragmentHost) {
        self.js_ctx.element_tree.borrow_mut().insert_fragment(host);
    }

    /// Append `child` to `parent`. The child id may reference a real element
    /// node or a fragment — `append_child` auto-detects the variant.
    pub fn link_child(&self, parent: NodeId, child: NodeId) {
        self.js_ctx
            .element_tree
            .borrow_mut()
            .append_child(parent, child);
        self.js_ctx.element_tree.borrow_mut().mark_dirty(parent);
        self.js_ctx.set_dirty();
    }

    /// Insert `child` into `parent` immediately before the existing node
    /// `ref_child`. Used by `LazyListElement::process_remount` to keep tree
    /// children ordered by logical item index when scrolling up (otherwise
    /// newly-built lower-index items would land at the end of the children
    /// vector and break layout / hit-testing).
    pub fn link_child_before(
        &self,
        parent: ElementNodeId,
        child: NodeId,
        ref_child: ElementNodeId,
    ) {
        self.js_ctx
            .element_tree
            .borrow_mut()
            .insert_before(parent, child, ref_child);
        self.js_ctx
            .element_tree
            .borrow_mut()
            .mark_dirty(parent.into());
        self.js_ctx.set_dirty();
    }

    /// Reorder an already-linked `child` under `parent` so that it sits
    /// immediately before `ref_child` in the parent's children vector.
    /// Equivalent to `remove_child` + `insert_before` but keeps the child's
    /// `parent` pointer intact. Used after `View::build` (which already
    /// appends the new child) to splice it into the correct slot when
    /// scrolling up mounts lower-index items.
    pub fn move_child_before(&self, parent: ElementNodeId, child: NodeId, ref_child: NodeId) {
        self.js_ctx
            .element_tree
            .move_child_before(parent, child, ref_child);
        self.js_ctx.set_dirty();
    }

    /// Remove `child` from its parent (does not delete the node).
    pub fn unlink_child(&self, parent: NodeId, child: NodeId) {
        self.js_ctx
            .element_tree
            .borrow_mut()
            .remove_child(parent, child);
        self.js_ctx.element_tree.borrow_mut().mark_dirty(parent);
        self.js_ctx.set_dirty();
    }

    /// Recursively remove a node and all its descendants from the tree.
    pub fn destroy_subtree(&self, id: ElementNodeId) {
        self.js_ctx.element_tree.destroy_subtree(id);
        self.js_ctx.set_dirty();
    }

    /// Destroy a subtree rooted at a node id (handles both real elements and
    /// fragments — dispatches via `is_fragment`).
    pub fn destroy_child(&self, id: NodeId) {
        self.js_ctx.element_tree.destroy_child(id);
        self.js_ctx.set_dirty();
    }

    /// Recursively remove a fragment and all its descendants from the tree.
    pub fn destroy_fragment(&self, id: crate::core::element::FragmentNodeId) {
        self.js_ctx.element_tree.destroy_fragment(id);
        self.js_ctx.set_dirty();
    }

    /// Build a child view under `parent` and return the resulting node id.
    pub fn build_child<Cx: ViewCx>(
        cx: &mut Cx,
        view: &dyn View,
        boa: &mut Context,
        parent: NodeId,
    ) -> NodeId {
        view.build(cx, boa, parent)
    }

    /// Mark a node dirty (needs re-layout + re-paint).
    pub fn mark_dirty(&self, id: NodeId) {
        self.js_ctx.element_tree.mark_dirty(id);
        self.js_ctx.set_dirty();
    }

    /// Set the query-key on a tree node (for test selectors).
    pub fn set_query_key(&self, id: ElementNodeId, keys: Vec<String>) {
        let mut tree = self.js_ctx.element_tree.borrow_mut();
        if let Some(node) = tree.get_element_mut(id) {
            node.query_key = if keys.is_empty() { None } else { Some(keys) };
        }
        tree.mark_dirty(id.into());
        drop(tree);
        self.js_ctx.set_dirty();
    }

    /// Read the computed layout of a node (for scroll controllers etc.).
    pub fn computed_layout(
        &self,
        id: ElementNodeId,
    ) -> Option<crate::core::layout::ComputedLayout> {
        self.js_ctx
            .element_tree
            .get_element(id)
            .map(|n| n.computed_layout)
    }

    /// Resolve pending focus/blur notifications recorded by `FocusManager`.
    /// Phase 1 enqueues JS mutations (on_focus / on_blur); Phase 2 fires
    /// Rust-level `on_focus_changed` lifecycle callbacks on each affected
    /// element, giving them a chance to spawn/cancel async tasks tied to
    /// focus state (e.g. caret blink).
    pub fn flush_focus_notifications(&mut self, boa: &mut Context) {
        let focus_changes = {
            let tree = self.js_ctx.element_tree.borrow();
            let mut focus = self.js_ctx.focus_manager.borrow_mut();
            let mut queue = self.js_ctx.mutation_queue.borrow_mut();
            focus.flush_pending(&tree, &mut queue)
        };
        if focus_changes.is_empty() {
            return;
        }
        for (id, focused) in &focus_changes {
            let mut element = {
                let mut tree = self.js_ctx.element_tree.borrow_mut();
                tree.get_element_mut(*id).and_then(|n| n.element.take())
            };
            if let Some(ref mut elem) = element {
                elem.run_on_focus_changed(*focused, self, boa);
            }
            if let Some(elem) = element {
                let mut tree = self.js_ctx.element_tree.borrow_mut();
                if let Some(node) = tree.get_element_mut(*id) {
                    node.element = Some(elem);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ViewCx for SharedViewCx — delegates to the inherent helpers above. This is the
// non-layout build context (interior mutability via the shared `NodeTree`).
// A layout-backed adapter implements the same trait against a direct
// `&mut NodeTreeData` borrow (added in a later phase).
// ---------------------------------------------------------------------------

impl ViewCx for SharedViewCx {
    fn alloc_node(&mut self) -> NodeId {
        SharedViewCx::alloc_node(self)
    }
    fn insert_node(&mut self, id: ElementNodeId, element: AnyElement, boa: &mut Context) {
        SharedViewCx::insert_node(self, id, element, boa);
    }
    fn insert_fragment(&mut self, host: FragmentHost) {
        SharedViewCx::insert_fragment(self, host);
    }
    fn link_child(&mut self, parent: NodeId, child: NodeId) {
        SharedViewCx::link_child(self, parent, child);
    }
    fn link_child_before(
        &mut self,
        parent: ElementNodeId,
        child: NodeId,
        ref_child: ElementNodeId,
    ) {
        SharedViewCx::link_child_before(self, parent, child, ref_child);
    }
    fn move_child_before(&mut self, parent: ElementNodeId, child: NodeId, ref_child: NodeId) {
        SharedViewCx::move_child_before(self, parent, child, ref_child);
    }
    fn destroy_child(&mut self, id: NodeId) {
        SharedViewCx::destroy_child(self, id);
    }
    fn mark_dirty(&mut self, id: NodeId) {
        SharedViewCx::mark_dirty(self, id);
    }
    fn set_query_key(&mut self, id: ElementNodeId, keys: Vec<String>) {
        SharedViewCx::set_query_key(self, id, keys);
    }
    fn computed_layout(&self, id: ElementNodeId) -> Option<crate::core::layout::ComputedLayout> {
        SharedViewCx::computed_layout(self, id)
    }
    fn store_read_only(&self) -> ReactiveReadStore {
        SharedViewCx::store_read_only(self)
    }
    fn subscribe_fragment(&self, id: FragmentNodeId) -> SubscribeCx {
        SharedViewCx::subscribe_fragment(self, id)
    }
    fn node_tree(&self) -> NodeTree {
        controller_handles(&self.js_ctx).0
    }
    fn mutation_queue(
        &self,
    ) -> std::rc::Rc<std::cell::RefCell<crate::core::edgy::mutation::PendingMutationInvocationQueue>>
    {
        controller_handles(&self.js_ctx).1
    }
    fn dirty(&self) -> std::rc::Rc<std::cell::Cell<bool>> {
        controller_handles(&self.js_ctx).2
    }
}
