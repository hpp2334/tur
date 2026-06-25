
use boa_engine::{Context, JsValue};

use crate::core::bridge::TurJsContext;
use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementObject};
use crate::core::reactive::{Readable, Store};

/// Context for building specs into the ElementTree and running effects.
/// Provides scoped access to the tree and the reactive store.
///
/// The boa `Context` is passed alongside (not stored) so callers can reborrow
/// freely while holding `&mut WidgetCx` — same pattern as the old `EdgyContext`.
pub struct WidgetCx {
    pub(crate) js_ctx: TurJsContext,
}

impl WidgetCx {
    pub fn new(js_ctx: TurJsContext) -> Self {
        WidgetCx { js_ctx }
    }

    pub fn js_ctx(&self) -> &TurJsContext {
        &self.js_ctx
    }

    // ----- reactive store -----------------------------------------------------

    pub fn store(&self) -> Store {
        self.js_ctx.store.clone()
    }

    /// Read an atom's current value as a raw `JsValue` (untracked).
    pub fn read_atom_raw<T>(&self, readable: Readable<T>, boa: &mut Context) -> JsValue {
        self.js_ctx.store.read(readable, boa)
    }

    /// Resolve a `Val<T>` to its current `T` value.  For reactive vals the
    /// atom is lazily read from the store (untracked).  Used during the effect
    /// phase; layout uses `LayoutContext::read_val` (with subscriber tracking).
    pub fn read_val<T: crate::core::widget::PropValue>(
        &self,
        val: &crate::core::widget::Val<T>,
        boa: &mut Context,
    ) -> Option<T> {
        use crate::core::widget::Val;
        match val {
            Val::Static(t) => Some(t.clone()),
            Val::Reactive(readable) => {
                let js = self.js_ctx.store.read(*readable, boa);
                T::from_js(&js)
            }
        }
    }

    // ----- ElementTree helpers ------------------------------------------------

    /// Allocate a fresh node id.
    pub fn alloc_node(&self) -> ElementNodeId {
        self.js_ctx.element_tree.borrow_mut().alloc_id()
    }

    /// Create an `AnyElement`-backed tree node and insert it (no parent yet).
    pub fn insert_node(&self, id: ElementNodeId, element: AnyElement, boa: &mut Context) {
        let node = ElementObject::new(id, element, boa);
        self.js_ctx.element_tree.borrow_mut().insert(node);
    }

    /// Append `child` to `parent`.
    pub fn link_child(&self, parent: ElementNodeId, child: ElementNodeId) {
        self.js_ctx
            .element_tree
            .borrow_mut()
            .append_child(parent, child);
        self.js_ctx.element_tree.borrow_mut().mark_dirty(parent);
        self.js_ctx.dirty.set(true);
    }

    /// Insert `child` into `parent` immediately before the existing node
    /// `ref_child`. Used by `LazyListElement::process_remount` to keep tree
    /// children ordered by logical item index when scrolling up (otherwise
    /// newly-built lower-index items would land at the end of the children
    /// vector and break layout / hit-testing).
    pub fn link_child_before(
        &self,
        parent: ElementNodeId,
        child: ElementNodeId,
        ref_child: ElementNodeId,
    ) {
        self.js_ctx
            .element_tree
            .borrow_mut()
            .insert_before(parent, child, ref_child);
        self.js_ctx.element_tree.borrow_mut().mark_dirty(parent);
        self.js_ctx.dirty.set(true);
    }

    /// Reorder an already-linked `child` under `parent` so that it sits
    /// immediately before `ref_child` in the parent's children vector.
    /// Equivalent to `remove_child` + `insert_before` but keeps the child's
    /// `parent` pointer intact. Used by `LazyListElement::process_remount`
    /// after `spec.build` (which already appends the new child) to splice
    /// it into the correct slot when scrolling up mounts lower-index items.
    pub fn move_child_before(
        &self,
        parent: ElementNodeId,
        child: ElementNodeId,
        ref_child: ElementNodeId,
    ) {
        let mut tree = self.js_ctx.element_tree.borrow_mut();
        // Remove the child from its current slot (if present) without
        // clearing its parent pointer, then re-insert at the right place.
        if let Some(node) = tree.get_mut(parent) {
            if let Some(pos) = node.children.iter().position(|&id| id == child) {
                node.children.remove(pos);
            }
        }
        if let Some(node) = tree.get_mut(parent) {
            if let Some(pos) = node.children.iter().position(|&id| id == ref_child) {
                node.children.insert(pos, child);
            } else {
                node.children.push(child);
            }
        }
        tree.mark_dirty(parent);
        drop(tree);
        self.js_ctx.dirty.set(true);
    }

    /// Remove `child` from its parent (does not delete the node).
    pub fn unlink_child(&self, parent: ElementNodeId, child: ElementNodeId) {
        self.js_ctx
            .element_tree
            .borrow_mut()
            .remove_child(parent, child);
        self.js_ctx.element_tree.borrow_mut().mark_dirty(parent);
        self.js_ctx.dirty.set(true);
    }

    /// Recursively remove a node and all its descendants from the tree.
    pub fn destroy_subtree(&self, id: ElementNodeId) {
        let family = self
            .js_ctx
            .element_tree
            .borrow()
            .get(id)
            .map(|n| (n.parent, n.children.clone()));
        if let Some((parent, children)) = family {
            if let Some(p) = parent {
                self.js_ctx
                    .element_tree
                    .borrow_mut()
                    .remove_child(p, id);
                self.js_ctx.element_tree.borrow_mut().mark_dirty(p);
            }
            for c in children {
                self.destroy_subtree(c);
            }
        }
        let _ = self.js_ctx.element_tree.borrow_mut().remove(id);
        self.js_ctx.dirty.set(true);
    }

    /// Build a child spec under `parent` and return the resulting node id.
    pub fn build_child(
        &mut self,
        spec: &dyn crate::core::widget::Component,
        boa: &mut Context,
        parent: ElementNodeId,
    ) -> ElementNodeId {
        spec.build(self, boa, parent)
    }

    /// Mark a node dirty (needs re-layout + re-paint).
    pub fn mark_dirty(&self, id: ElementNodeId) {
        self.js_ctx.element_tree.borrow_mut().mark_dirty(id);
        self.js_ctx.dirty.set(true);
    }

    /// Set the query-key on a tree node (for test selectors).
    pub fn set_query_key(&self, id: ElementNodeId, keys: Vec<String>) {
        let mut tree = self.js_ctx.element_tree.borrow_mut();
        if let Some(node) = tree.get_mut(id) {
            node.query_key = if keys.is_empty() { None } else { Some(keys) };
        }
        tree.mark_dirty(id);
        self.js_ctx.dirty.set(true);
    }

    /// Read the computed layout of a node (for scroll controllers etc.).
    pub fn computed_layout(&self, id: ElementNodeId) -> Option<tur_shared::ComputedLayout> {
        self.js_ctx
            .element_tree
            .borrow()
            .get(id)
            .map(|n| n.computed_layout)
    }
}
