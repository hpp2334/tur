use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::edgy::reactive::ReactiveReadStore;
use crate::core::edgy::value::FromValue;
use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::elements::{AnyElement, FragmentHost, NodeTree};
use crate::core::instance::InstanceContext;
use crate::core::layout::SubscribeCx;
use crate::core::view::Val;

// ---------------------------------------------------------------------------
// ViewCx — the build capability a `View::build` impl needs to mount itself
// into the node tree.
//
// `SharedViewCx` (the normal, non-layout build context) implements this via its
// interior-mutability helpers. A layout-backed adapter (added in a later
// phase) implements the same trait against a `&mut NodeTreeData` borrow, so
// `View::build` can run *during layout* through whichever borrow the caller
// holds — without `View` knowing which.
//
// Object-safety: this trait is used as `&mut dyn ViewCx` (so that `View` stays
// object-safe: `View::build` takes `&mut dyn ViewCx`, no generics). Generic
// helpers that don't fit a vtable (`read_val<T>`) live as free functions
// below, taking `&mut dyn ViewCx` for the store handle.
// ---------------------------------------------------------------------------

pub trait ViewCx {
    /// Allocate a fresh node id.
    fn alloc_node(&mut self) -> NodeId;

    /// Create an `AnyElement`-backed tree node and insert it (no parent yet).
    fn insert_node(&mut self, id: ElementNodeId, element: AnyElement);

    /// Insert a `FragmentHost` into the fragments map.
    fn insert_fragment(&mut self, host: FragmentHost);

    /// Append `child` to `parent` (auto-detects element vs fragment).
    fn link_child(&mut self, parent: NodeId, child: NodeId);

    /// Insert `child` into `parent` immediately before the existing
    /// `ref_child`. Used to keep tree children ordered by logical index.
    fn link_child_before(&mut self, parent: ElementNodeId, child: NodeId, ref_child: ElementNodeId);

    /// Reorder an already-linked `child` under `parent` so that it sits
    /// immediately before `ref_child`.
    fn move_child_before(&mut self, parent: ElementNodeId, child: NodeId, ref_child: NodeId);

    /// Destroy a subtree rooted at `id` (handles both elements and fragments).
    fn destroy_child(&mut self, id: NodeId);

    /// Mark a node dirty (needs re-layout + re-paint).
    fn mark_dirty(&mut self, id: NodeId);

    /// Set the query-key on a tree node (for test selectors).
    fn set_query_key(&mut self, id: ElementNodeId, keys: Vec<String>);

    /// Read the computed layout of a node (for scroll controllers etc.).
    fn computed_layout(&self, id: ElementNodeId) -> Option<crate::core::layout::ComputedLayout>;

    /// Read-only view of the reactive store for resolving atom values.
    fn store_read_only(&self) -> ReactiveReadStore;

    /// Create a `SubscribeCx` scoped to a fragment, so the fragment can
    /// declare its reactive atom deps at build time.
    fn subscribe_fragment(&self, id: FragmentNodeId) -> SubscribeCx;

    // ----- shared handle access (controller binding, e.g. ScrollView) --------

    /// A clonable handle to the node tree (for controllers that reach the
    /// tree at event time, outside layout).
    fn node_tree(&self) -> NodeTree;

    /// The pending-mutation queue handle (for controllers that fire mutations).
    fn mutation_queue(&self) -> Rc<RefCell<PendingMutationInvocationQueue>>;

    /// The dirty-flag handle (for controllers that request paints).
    fn dirty(&self) -> Rc<Cell<bool>>;
}

// ---------------------------------------------------------------------------
// Generic helpers — free functions (not on the trait) so `ViewCx` stays
// object-safe. They take `&mut dyn ViewCx` for the store handle + realm.
// ---------------------------------------------------------------------------

/// Resolve a `Val<T>` to its current `T` value. For reactive vals the atom is
/// lazily read from the store (untracked) as a native `Value` and decoded via
/// [`FromValue`].
pub fn read_val<T: FromValue + Clone + 'static>(cx: &mut dyn ViewCx, val: &Val<T>) -> Option<T> {
    match val {
        Val::Static(t) => Some(t.clone()),
        Val::Reactive(readable) => {
            let store = cx.store_read_only();
            let value = store.read(*readable);
            T::from_value(&value).ok()
        }
    }
}

/// Convenience: resolve an `Option<Val<T>>` (absent → `None`).
pub fn read_val_opt<T: FromValue + Clone + 'static>(
    cx: &mut dyn ViewCx,
    val: Option<&Val<T>>,
) -> Option<T> {
    val.and_then(|v| read_val(cx, v))
}

/// Borrow the shared handles a controller needs, from an [`InstanceContext`].
pub fn controller_handles(
    js_ctx: &InstanceContext,
) -> (
    NodeTree,
    Rc<RefCell<PendingMutationInvocationQueue>>,
    Rc<Cell<bool>>,
) {
    (
        js_ctx.element_tree.clone(),
        js_ctx.mutation_queue.clone(),
        js_ctx.dirty.clone(),
    )
}
