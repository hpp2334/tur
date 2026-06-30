use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::reactive::ReactiveReadJsContext;
use parley::{FontContext, LayoutContext as ParleyLayoutContext};
use tur_shared::{Constraints, Offset, Size};

use crate::core::edgy_event::PendingMutationInvocationQueue;
use crate::core::element::ElementNodeId;
use crate::core::elements::{NodeTree, NodeTreeData};
use crate::core::fonts::FontManager;
use crate::core::resource::{ResourceId, ResourceMap};
use crate::core::view::{PropValue, Val};
use crate::elements::ExpandedElement;

pub struct LayoutContext<'a, 'js> {
    pub(crate) tree: &'a mut NodeTreeData,
    node_id: ElementNodeId,
    font_manager: &'a mut FontManager,
    text_layout_cx: &'a mut ParleyLayoutContext<[u8; 4]>,
    resource_map: &'a ResourceMap,
    /// Shared handles needed to build a `LayoutViewCx` for layout-phase
    /// mount/unmount (LazyList remount). The `node_tree` is a clonable
    /// handle so controllers captured at build time can reach the tree at
    /// event time; `mutation_queue` / `dirty` let built views request
    /// redraws and enqueue mutations.
    pub(crate) node_tree: NodeTree,
    pub(crate) mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub(crate) dirty: Rc<Cell<bool>>,
    /// Read-only JS engine face. Held so `read_val` can (lazily) recompute
    /// stale derived atoms; this is the only JS access layout has, and the face
    /// exposes **only** `read` — no `set` / mutation is reachable from layout.
    /// `'js` is the lifetime of the borrowed JS `Context` (independent of the
    /// tree/manager borrow `'a` so the face can be re-borrowed recursively).
    pub(crate) js: &'a mut ReactiveReadJsContext<'js>,
}

impl<'a, 'js> LayoutContext<'a, 'js> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        tree: &'a mut NodeTreeData,
        node_id: ElementNodeId,
        font_manager: &'a mut FontManager,
        text_layout_cx: &'a mut ParleyLayoutContext<[u8; 4]>,
        resource_map: &'a ResourceMap,
        node_tree: NodeTree,
        mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
        dirty: Rc<Cell<bool>>,
        js: &'a mut ReactiveReadJsContext<'js>,
    ) -> Self {
        LayoutContext {
            tree,
            node_id,
            font_manager,
            text_layout_cx,
            resource_map,
            node_tree,
            mutation_queue,
            dirty,
            js,
        }
    }

    pub fn layout_child(&mut self, child_id: ElementNodeId, constraints: &Constraints) -> Size {
        self.tree.layout(
            child_id,
            constraints,
            self.font_manager,
            self.text_layout_cx,
            self.resource_map,
            self.node_tree.clone(),
            self.mutation_queue.clone(),
            self.dirty.clone(),
            self.js,
        )
    }

    pub fn set_child_offset(&mut self, child_id: ElementNodeId, offset: Offset) {
        if let Some(node) = self.tree.elements.get_mut(&child_id) {
            node.computed_layout.offset = offset;
        }
    }

    pub fn set_child_offset_self(&mut self, offset: Offset) {
        if let Some(node) = self.tree.elements.get_mut(&self.node_id) {
            node.computed_layout.offset = offset;
        }
    }

    pub fn child_type_name(&self, child_id: ElementNodeId) -> &'static str {
        self.tree
            .elements
            .get(&child_id)
            .and_then(|n| n.element.as_ref())
            .map(|e| e.type_name())
            .unwrap_or("tur_container")
    }

    pub fn child_computed_size(&self, child_id: ElementNodeId) -> Size {
        self.tree
            .elements
            .get(&child_id)
            .map(|n| n.computed_layout.size)
            .unwrap_or(Size::ZERO)
    }

    /// The current node's own computed size (set by the driver after a
    /// `perform_layout` call returns, and readable by later phases / parents
    /// via `child_computed_size`). Reading this from inside an element's own
    /// `perform_layout` returns a stale value — use the locally-computed size
    /// instead.
    pub fn self_computed_size(&self) -> Size {
        self.tree
            .elements
            .get(&self.node_id)
            .map(|n| n.computed_layout.size)
            .unwrap_or(Size::ZERO)
    }

    pub fn child_element<T: 'static>(&self, child_id: ElementNodeId) -> Option<&T> {
        self.tree
            .elements
            .get(&child_id)
            .and_then(|n| n.element.as_ref())
            .and_then(|e| e.cast::<T>())
    }

    /// Resolve the `flex` weight of a flex-item child (`Expanded({ flex })`).
    /// Returns 0.0 if the child is not an `Expanded` element. If it is an
    /// `Expanded` but the `flex` prop is absent, returns 1.0 (Flutter default).
    pub fn child_flex(&mut self, child_id: ElementNodeId) -> f64 {
        let Some(expanded) = self.child_element::<ExpandedElement>(child_id) else {
            return 0.0;
        };
        let Some(flex_val) = expanded.view.flex.clone() else {
            return 1.0;
        };
        self.read_val(&flex_val).unwrap_or(1.0).max(0.0)
    }

    pub fn text_layout_contexts(
        &mut self,
    ) -> (&mut FontContext, &mut ParleyLayoutContext<[u8; 4]>) {
        (self.font_manager.font_context(), self.text_layout_cx)
    }

    pub fn get_image_natural_size(&self, resource_id: ResourceId) -> Option<Size> {
        self.resource_map.get_image(resource_id).map(|r| r.natural_size)
    }

    /// Resolve a `Val<T>` to its current `T` value. For reactive vals the atom
    /// is read through the read-only JS face. Subscription is **not**
    /// established here — it is declared explicitly in the element's
    /// `subscribe` phase (see [`crate::core::layout::ElementSubscribe`]).
    ///
    /// Returns `None` if the prop is absent (`Option<Val<T>>::None`) or the
    /// atom value can't be decoded as `T`.
    pub fn read_val<T: PropValue>(&mut self, val: &Val<T>) -> Option<T> {
        match val {
            Val::Static(t) => Some(t.clone()),
            Val::Reactive(readable) => {
                let js = self.js.read(*readable);
                T::from_js(&js)
            }
        }
    }

    /// Convenience: resolve an `Option<Val<T>>` (absent → `None`).
    pub fn read_val_opt<T: PropValue>(&mut self, val: Option<&Val<T>>) -> Option<T> {
        val.and_then(|v| self.read_val(v))
    }
}
