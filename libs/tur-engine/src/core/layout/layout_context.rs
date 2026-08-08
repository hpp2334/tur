use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::edgy::reactive::ReactiveReadJsContext;
use crate::core::layout::{Constraints, Offset, Size};
use parley::{FontContext, LayoutContext as ParleyLayoutContext};

use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::element::ElementNodeId;
use crate::core::elements::{NodeTree, NodeTreeData};
use crate::core::fonts::FontManager;
use crate::core::image_resource::{ImageManager, ImageResourceId};
use crate::core::view::{FromJs, Val};

pub struct LayoutContext<'a, 'js> {
    pub tree: &'a mut NodeTreeData,
    node_id: ElementNodeId,
    font_manager: &'a mut FontManager,
    text_layout_cx: &'a mut ParleyLayoutContext<[u8; 4]>,
    image_manager: &'a ImageManager,
    /// Shared handles needed to build a `LayoutViewCx` for layout-phase
    /// mount/unmount (LazyList remount). The `node_tree` is a clonable
    /// handle so controllers captured at build time can reach the tree at
    /// event time; `mutation_queue` / `dirty` let built views request
    /// paints and enqueue mutations.
    pub node_tree: NodeTree,
    pub mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub dirty: Rc<Cell<bool>>,
    /// Read-only JS engine face. Held so `read_val` can (lazily) recompute
    /// stale derived atoms; this is the only JS access layout has, and the face
    /// exposes **only** `read` — no `set` / mutation is reachable from layout.
    /// `'js` is the lifetime of the borrowed JS `Context` (independent of the
    /// tree/manager borrow `'a` so the face can be re-borrowed recursively).
    pub js: &'a mut ReactiveReadJsContext<'js>,
}

impl<'a, 'js> LayoutContext<'a, 'js> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tree: &'a mut NodeTreeData,
        node_id: ElementNodeId,
        font_manager: &'a mut FontManager,
        text_layout_cx: &'a mut ParleyLayoutContext<[u8; 4]>,
        image_manager: &'a ImageManager,
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
            image_manager,
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
            self.image_manager,
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

    pub fn text_layout_contexts(
        &mut self,
    ) -> (&mut FontContext, &mut ParleyLayoutContext<[u8; 4]>) {
        (self.font_manager.font_context(), self.text_layout_cx)
    }

    pub fn get_image_natural_size(&self, image_resource_id: ImageResourceId) -> Option<Size> {
        self.image_manager.get(image_resource_id).map(|m| m.size)
    }

    /// Resolve a `Val<T>` to its current `T` value. For reactive vals the atom
    /// is read through the read-only JS face. Subscription is **not**
    /// established here — it is declared explicitly in the element's
    /// `subscribe` phase (see [`crate::core::layout::ElementSubscribe`]).
    ///
    /// Returns `None` if the prop is absent (`Option<Val<T>>::None`) or the
    /// atom value can't be decoded as `T`.
    pub fn read_val<T: FromJs + Clone + 'static>(&mut self, val: &Val<T>) -> Option<T> {
        match val {
            Val::Static(t) => Some(t.clone()),
            Val::Reactive(readable) => {
                let js = self.js.read(*readable);
                T::from_js(&js).ok()
            }
        }
    }

    /// Convenience: resolve an `Option<Val<T>>` (absent → `None`).
    pub fn read_val_opt<T: FromJs + Clone + 'static>(&mut self, val: Option<&Val<T>>) -> Option<T> {
        val.and_then(|v| self.read_val(v))
    }
}
