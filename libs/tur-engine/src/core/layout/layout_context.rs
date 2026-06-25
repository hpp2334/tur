use boa_engine::Context;
use parley::{FontContext, LayoutContext as ParleyLayoutContext};
use tur_shared::{Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::fonts::FontManager;
use crate::core::resource::{ResourceId, ResourceMap};
use crate::core::widget::{PropValue, Val};
use crate::elements::ExpandedElement;

pub struct LayoutContext<'a> {
    pub(crate) tree: &'a mut ElementTree,
    node_id: ElementNodeId,
    font_manager: &'a mut FontManager,
    text_layout_cx: &'a mut ParleyLayoutContext<[u8; 4]>,
    resource_map: &'a ResourceMap,
    /// The boa JS runtime. Held so `read_val` can (in later phases) recompute
    /// stale derived atoms lazily; layout is the only rendering phase with JS
    /// access. Paint never touches this.
    pub(crate) boa: &'a mut Context,
}

impl<'a> LayoutContext<'a> {
    pub(crate) fn new(
        tree: &'a mut ElementTree,
        node_id: ElementNodeId,
        font_manager: &'a mut FontManager,
        text_layout_cx: &'a mut ParleyLayoutContext<[u8; 4]>,
        resource_map: &'a ResourceMap,
        boa: &'a mut Context,
    ) -> Self {
        LayoutContext {
            tree,
            node_id,
            font_manager,
            text_layout_cx,
            resource_map,
            boa,
        }
    }

    pub fn layout_child(&mut self, child_id: ElementNodeId, constraints: &Constraints) -> Size {
        self.tree.layout(
            child_id,
            constraints,
            self.font_manager,
            self.text_layout_cx,
            self.resource_map,
            self.boa,
        )
    }

    pub fn set_child_offset(&mut self, child_id: ElementNodeId, offset: Offset) {
        if let Some(node) = self.tree.nodes.get_mut(&child_id) {
            node.computed_layout.offset = offset;
        }
    }

    pub fn set_child_offset_self(&mut self, offset: Offset) {
        if let Some(node) = self.tree.nodes.get_mut(&self.node_id) {
            node.computed_layout.offset = offset;
        }
    }

    pub fn child_type_name(&self, child_id: ElementNodeId) -> &'static str {
        self.tree
            .nodes
            .get(&child_id)
            .and_then(|n| n.element.as_ref())
            .map(|e| e.type_name())
            .unwrap_or("tur_container")
    }

    pub fn child_computed_size(&self, child_id: ElementNodeId) -> Size {
        self.tree
            .nodes
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
            .nodes
            .get(&self.node_id)
            .map(|n| n.computed_layout.size)
            .unwrap_or(Size::ZERO)
    }

    pub fn child_element<T: 'static>(&self, child_id: ElementNodeId) -> Option<&T> {
        self.tree
            .nodes
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
        let Some(flex_val) = expanded.component.flex.clone() else {
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

    /// Resolve a `Val<T>` to its current `T` value.  For reactive vals the
    /// atom is read from the store (no boa `Context` needed) and the
    /// dependency `(atom, subscriber)` is recorded so a future flush can mark
    /// this node dirty.
    ///
    /// Returns `None` if the prop is absent (`Option<Val<T>>::None`) or the
    /// atom value can't be decoded as `T`.
    pub fn read_val<T: PropValue>(&mut self, val: &Val<T>) -> Option<T> {
        match val {
            Val::Static(t) => Some(t.clone()),
            Val::Reactive(readable) => {
                let store = self.tree.store.as_ref()?;
                let sub = crate::core::reactive::SubscriberId::new(self.node_id.as_u64());
                let _guard = store.subscribe_scope(sub);
                let js = store.read(*readable, self.boa);
                T::from_js(&js)
            }
        }
    }

    /// Convenience: resolve an `Option<Val<T>>` (absent → `None`).
    pub fn read_val_opt<T: PropValue>(&mut self, val: Option<&Val<T>>) -> Option<T> {
        val.and_then(|v| self.read_val(v))
    }
}
