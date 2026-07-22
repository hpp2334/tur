use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use parley::LayoutContext as ParleyLayoutContext;
use crate::core::layout::{Constraints, Offset, Size};

use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementObject, FragmentHost, TraceValue};
use crate::core::fonts::FontManager;
use crate::core::image_resource::ImageResourceMap;
use crate::core::layout::{LayoutContext, SubscribeCx};
use crate::core::edgy::reactive::{ReactiveReadStore, ReactiveReadJsContext, Store, SubscriberId};
use crate::core::render::{Canvas, PaintContext};
use crate::core::shell::PaintShell;

pub struct NodeTreeData {
    pub(crate) elements: HashMap<ElementNodeId, ElementObject>,
    /// Control-flow primitives (Each / Condition / Switch). Keyed by id (same
    /// counter as `elements`). Fragments have no `AnyElement` and are never laid
    /// out / painted directly — `flatten_children` splices their children into
    /// the enclosing flex's layout.
    fragments: HashMap<FragmentNodeId, FragmentHost>,
    root_id: Option<ElementNodeId>,
    next_id: u64,
    pub(crate) store: Store,
    /// Cached read-only reactive face; the layout driver wraps this in a
    /// [`ReactiveReadJsContext`] (with a `Context` borrow) so layout can only
    /// read atoms, never `set` / mutate.
    pub(crate) read_face: ReactiveReadStore,
    /// Element ids inserted since the last lifecycle flush. Drained by the
    /// flush loop, which fires each element's `on_mounted` hook.
    pending_mounted: Vec<ElementNodeId>,
    /// Elements removed (taken out) since the last lifecycle flush. Drained by
    /// the flush loop, which fires each element's `before_destroy` hook before
    /// dropping it. Keeping them here (rather than dropping immediately) lets
    /// the hook run with a live element + mutation queue in scope.
    pending_destroy: Vec<AnyElement>,
}


impl NodeTreeData {
    pub fn new(store: Store) -> Self {
        let read_face = store.read_only();
        NodeTreeData {
            elements: HashMap::new(),
            fragments: HashMap::new(),
            root_id: None,
            next_id: 1,
            store,
            read_face,
            pending_mounted: Vec::new(),
            pending_destroy: Vec::new(),
        }
    }

    pub fn alloc_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        NodeId::new(id)
    }

    pub fn element_count(&self) -> usize {
        self.elements.len()
    }

    /// All element node ids (snapshot). Used by the flush loop to iterate
    /// effects without holding the tree borrow across the effect call.
    pub fn element_ids(&self) -> Vec<ElementNodeId> {
        self.elements.keys().copied().collect()
    }

    /// Take the pending-mounted ids (for the `on_mounted` flush).
    pub fn take_pending_mounted(&mut self) -> Vec<ElementNodeId> {
        std::mem::take(&mut self.pending_mounted)
    }

    /// Take the pending-destroyed elements (for the `before_destroy` flush).
    pub fn take_pending_destroy(&mut self) -> Vec<AnyElement> {
        std::mem::take(&mut self.pending_destroy)
    }

    pub fn insert_element(&mut self, element: ElementObject) {
        if self.root_id.is_none() {
            self.root_id = Some(element.id);
        }
        // Record for the `on_mounted` lifecycle flush.
        self.pending_mounted.push(element.id);
        self.elements.insert(element.id, element);
    }

    pub fn get_element(&self, id: ElementNodeId) -> Option<&ElementObject> {
        self.elements.get(&id)
    }

    pub fn get_element_mut(&mut self, id: ElementNodeId) -> Option<&mut ElementObject> {
        self.elements.get_mut(&id)
    }

    pub fn remove_element(&mut self, id: ElementNodeId) -> Option<ElementObject> {
        let node = self.elements.remove(&id)?;
        if self.root_id == Some(id) {
            self.root_id = None;
        }
        Some(node)
    }

    pub fn root_element_id(&self) -> Option<ElementNodeId> {
        self.root_id
    }

    pub fn root_element(&self) -> Option<&ElementObject> {
        self.root_id.and_then(|id| self.elements.get(&id))
    }

    pub fn root_element_mut(&mut self) -> Option<&mut ElementObject> {
        self.root_id.and_then(|id| self.elements.get_mut(&id))
    }

    pub fn set_root_element(&mut self, id: ElementNodeId) {
        self.root_id = Some(id);
    }

    /// Insert a fragment host into the fragments map.
    pub fn insert_fragment(&mut self, host: FragmentHost) {
        self.fragments.insert(host.id, host);
    }

    /// Remove a fragment from the map.
    pub fn remove_fragment(&mut self, id: FragmentNodeId) -> Option<FragmentHost> {
        self.fragments.remove(&id)
    }

    pub fn get_fragment(&self, id: FragmentNodeId) -> Option<&FragmentHost> {
        self.fragments.get(&id)
    }

    pub fn get_fragment_mut(&mut self, id: FragmentNodeId) -> Option<&mut FragmentHost> {
        self.fragments.get_mut(&id)
    }

    /// True if `id` is a fragment (not a real element node).
    pub fn is_fragment(&self, id: NodeId) -> bool {
        self.fragments.contains_key(&FragmentNodeId::new(id.as_u64()))
    }

    /// Remove a `child_id` entry from a parent's children vec (node or fragment).
    pub fn remove_child_entry(&mut self, parent_id: NodeId, child_id: NodeId) {
        if let Some(node) = self.elements.get_mut(&ElementNodeId::new(parent_id.as_u64())) {
            node.children.retain(|c| *c != child_id);
        } else if let Some(frag) = self.fragments.get_mut(&FragmentNodeId::new(parent_id.as_u64())) {
            frag.children.retain(|c| *c != child_id);
        }
    }

    pub fn append_child(&mut self, parent_id: NodeId, child_id: NodeId) -> bool {
        // Guard: don't link to a parent that doesn't exist in either map
        // (e.g. the `temp_parent` placeholder in `tur_render` which is
        // allocated but never inserted). Matches the pre-fragment behavior.
        if !self.elements.contains_key(&ElementNodeId::new(parent_id.as_u64()))
            && !self.fragments.contains_key(&FragmentNodeId::new(parent_id.as_u64()))
        {
            return false;
        }
        // Set the child's parent pointer (node or fragment).
        if let Some(c) = self.elements.get_mut(&ElementNodeId::new(child_id.as_u64())) {
            c.parent = Some(parent_id);
        } else if let Some(f) = self.fragments.get_mut(&FragmentNodeId::new(child_id.as_u64())) {
            f.parent = parent_id;
        }
        // Push to the parent's children vec (node or fragment).
        if let Some(node) = self.elements.get_mut(&ElementNodeId::new(parent_id.as_u64())) {
            node.children.push(child_id);
        } else if let Some(frag) = self.fragments.get_mut(&FragmentNodeId::new(parent_id.as_u64())) {
            frag.children.push(child_id);
        }
        true
    }

    pub fn remove_child(&mut self, parent_id: NodeId, child_id: NodeId) -> bool {
        if let Some(node) = self.elements.get_mut(&ElementNodeId::new(parent_id.as_u64())) {
            node.children.retain(|c| *c != child_id);
        } else if let Some(frag) = self.fragments.get_mut(&FragmentNodeId::new(parent_id.as_u64())) {
            frag.children.retain(|c| *c != child_id);
        }
        // Clear the child's parent pointer (node or fragment).
        if let Some(c) = self.elements.get_mut(&ElementNodeId::new(child_id.as_u64())) {
            c.parent = None;
        }
        true
    }

    pub fn insert_before(
        &mut self,
        parent_id: ElementNodeId,
        child_id: NodeId,
        ref_id: ElementNodeId,
    ) -> bool {
        if !self.elements.contains_key(&parent_id)
            || (!self.elements.contains_key(&ElementNodeId::new(child_id.as_u64()))
                && !self.fragments.contains_key(&FragmentNodeId::new(child_id.as_u64())))
            || !self.elements.contains_key(&ref_id)
        {
            return false;
        }
        // Set the child's parent pointer.
        if let Some(c) = self.elements.get_mut(&ElementNodeId::new(child_id.as_u64())) {
            c.parent = Some(parent_id.into());
        } else if let Some(f) = self.fragments.get_mut(&FragmentNodeId::new(child_id.as_u64())) {
            f.parent = parent_id.into();
        }
        let insert_fn = |children: &mut Vec<NodeId>| {
            if let Some(pos) = children.iter().position(|c| *c == ref_id.into()) {
                children.insert(pos, child_id);
            } else {
                children.push(child_id);
            }
        };
        if let Some(node) = self.elements.get_mut(&parent_id) {
            insert_fn(&mut node.children);
        }
        true
    }

    /// Recursively expand fragment entries: a fragment's own children are
    /// spliced inline, so the enclosing flex lays them out directly as its
    /// own items (`display: contents`). Pure read. Returns only real element
    /// ids — fragments are recursed through, never included.
    fn flatten_children(&self, children: &[NodeId]) -> Vec<ElementNodeId> {
        let mut out = Vec::with_capacity(children.len());
        for &child in children {
            if self.is_fragment(child) {
                if let Some(frag) = self.fragments.get(&FragmentNodeId::new(child.as_u64())) {
                    out.extend(self.flatten_children(&frag.children));
                }
            } else {
                out.push(ElementNodeId::new(child.as_u64()));
            }
        }
        out
    }

    /// Convenience: flatten a real element's children.
    pub fn children_of_element(&self, id: ElementNodeId) -> Vec<ElementNodeId> {
        self.elements
            .get(&id)
            .map(|n| self.flatten_children(&n.children))
            .unwrap_or_default()
    }

    /// Raw (un-flattened) children of a node.
    pub fn raw_children_of_element(&self, id: ElementNodeId) -> Vec<NodeId> {
        self.elements
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default()
    }

    pub fn parent_of_element(&self, id: ElementNodeId) -> Option<NodeId> {
        self.elements.get(&id).and_then(|n| n.parent)
    }

    /// Recursively remove an element node and all its descendants.
    /// `dirty` is set on each removed subtree's parent (propagates to root).
    pub fn destroy_subtree(&mut self, id: ElementNodeId) {
        let family = self
            .elements
            .get(&id)
            .map(|n| (n.parent, n.children.clone()));
        if let Some((parent, children)) = family {
            if let Some(p) = parent {
                self.remove_child_entry(p, id.into());
                self.mark_dirty(p);
            }
            for c in children {
                self.destroy_child(c);
            }
        }
        // Capture the element so the flush loop can fire `before_destroy`.
        if let Some(node) = self.remove_element(id)
            && let Some(elem) = node.element {
                self.pending_destroy.push(elem);
            }
    }

    /// Destroy a subtree rooted at a node id (handles both real elements and
    /// fragments — dispatches via `is_fragment`).
    pub fn destroy_child(&mut self, id: NodeId) {
        if self.is_fragment(id) {
            self.destroy_fragment(FragmentNodeId::new(id.as_u64()));
        } else {
            self.destroy_subtree(ElementNodeId::new(id.as_u64()));
        }
    }

    /// Recursively remove a fragment and all its descendants.
    pub fn destroy_fragment(&mut self, id: FragmentNodeId) {
        let family = self
            .fragments
            .get(&id)
            .map(|f| (Some(f.parent), f.children.clone()));
        if let Some((parent, children)) = family {
            if let Some(p) = parent {
                self.remove_child_entry(p, id.into());
                self.mark_dirty(p);
            }
            for c in children {
                self.destroy_child(c);
            }
        }
        let _ = self.remove_fragment(id);
    }

    /// Insert `child` into `parent`'s children vec immediately before
    /// `ref_child` (no parent-pointer update — used to reorder an already-linked
    /// child, e.g. keeping LazyList items ordered by logical index).
    pub fn move_child_before(&mut self, parent: ElementNodeId, child: NodeId, ref_child: NodeId) {
        if let Some(node) = self.elements.get_mut(&parent)
            && let Some(pos) = node.children.iter().position(|c| *c == child) {
                node.children.remove(pos);
            }
        if let Some(node) = self.elements.get_mut(&parent) {
            if let Some(pos) = node.children.iter().position(|c| *c == ref_child) {
                node.children.insert(pos, child);
            } else {
                node.children.push(child);
            }
        }
        self.mark_dirty(parent.into());
    }

    pub fn mark_dirty(&mut self, id: NodeId) {
        // Propagate dirtiness from `id` up to the root, marking each **real**
        // element ancestor (short-circuiting on an already-dirty node).
        //
        // Fragments are never laid out, so they are **skipped**: dirtiness
        // passes straight through them to their `.parent` (a real element).
        // This is how a fragment's rebuild (via `effect`) reaches the enclosing
        // flex — `cx.mark_dirty(fragment.parent)` marks the real ancestor, and
        // the flex re-lays-out with the new flattened children.
        let mut path = Vec::new();
        {
            let mut current = Some(id);
            while let Some(cid) = current {
                if let Some(node) = self.elements.get(&ElementNodeId::new(cid.as_u64())) {
                    if node.dirty_layout {
                        break;
                    }
                    path.push(cid);
                    current = node.parent;
                } else if let Some(frag) = self.fragments.get(&FragmentNodeId::new(cid.as_u64())) {
                    // Skip fragments — hop to the real ancestor.
                    current = Some(frag.parent);
                } else {
                    break;
                }
            }
        }
        for cid in path {
            if let Some(node) = self.elements.get_mut(&ElementNodeId::new(cid.as_u64())) {
                node.dirty_layout = true;
            }
        }
    }

    pub fn mark_root_dirty(&mut self) {
        for node in self.elements.values_mut() {
            node.dirty_layout = true;
        }
    }

    pub fn has_dirty_layout(&self) -> bool {
        self.elements.values().any(|n| n.dirty_layout)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn compute_layout(
        &mut self,
        constraints: &Constraints,
        font_manager: &mut FontManager,
        text_layout_cx: &mut ParleyLayoutContext<[u8; 4]>,
        image_resource_map: &ImageResourceMap,
        node_tree: NodeTree,
        mutation_queue: std::rc::Rc<std::cell::RefCell<crate::core::edgy::mutation::PendingMutationInvocationQueue>>,
        dirty: std::rc::Rc<std::cell::Cell<bool>>,
        boa: &mut boa_engine::Context,
    ) -> Size {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return constraints.constrain(Size::ZERO),
        };

        let mut js = ReactiveReadJsContext::new(self.read_face.clone(), boa);
        self.layout(root_id, constraints, font_manager, text_layout_cx, image_resource_map, node_tree, mutation_queue, dirty, &mut js)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn layout<'a, 'js>(
        &'a mut self,
        id: ElementNodeId,
        constraints: &Constraints,
        font_manager: &'a mut FontManager,
        text_layout_cx: &'a mut ParleyLayoutContext<[u8; 4]>,
        image_resource_map: &'a ImageResourceMap,
        node_tree: NodeTree,
        mutation_queue: std::rc::Rc<std::cell::RefCell<crate::core::edgy::mutation::PendingMutationInvocationQueue>>,
        dirty: std::rc::Rc<std::cell::Cell<bool>>,
        js: &'a mut ReactiveReadJsContext<'js>,
    ) -> Size {
        let (is_dirty, constraints_changed) = self
            .elements
            .get(&id)
            .map(|n| (n.dirty_layout, n.last_constraints != Some(*constraints)))
            .unwrap_or((false, true));
        if !is_dirty && !constraints_changed {
            return self.elements.get(&id)
                .map(|n| n.computed_layout.size)
                .unwrap_or(Size::ZERO);
        }

        let direct = self
            .elements
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        // Flatten: splice fragment (Each / Condition / Switch) children
        // directly into the layout-children list so the enclosing flex lays
        // them out as its own items. Pure read — no tree mutation.
        let children = self.flatten_children(&direct);

        let mut element = self
            .elements
            .get_mut(&id)
            .and_then(|n| n.element.take())
            .expect("element missing during layout");

        let mut cx = LayoutContext::new(
            self,
            id,
            font_manager,
            text_layout_cx,
            image_resource_map,
            node_tree,
            mutation_queue,
            dirty,
            js,
        );
        // `perform_layout` measures the children (recursively laying each out
        // via `cx.layout_child`), computes this node's size, and assigns each
        // child's offset — all in one pass.
        let size = element.perform_layout(constraints, &children, &mut cx);

        // Explicit subscribe phase: re-declare this node's reactive deps so a
        // future reactive flush can mark it dirty. The `SubscribeCx` swap
        // (on drop) replaces the node's prior subscriptions.
        let sub_index = cx.tree.store.subscriber_index();
        let mut sub_cx = SubscribeCx::new(sub_index, SubscriberId::new(id.into()));
        element.subscribe(&mut sub_cx);

        let constrained = constraints.constrain(size);
        let node = cx.tree.elements.get_mut(&id).unwrap();
        node.element = Some(element);
        node.computed_layout.size = constrained;
        node.last_constraints = Some(*constraints);
        node.dirty_layout = false;
        constrained
    }

    pub fn paint(
        &self,
        canvas: &mut dyn Canvas,
        focused_node_id: Option<ElementNodeId>,
        image_resource_map: &ImageResourceMap,
        shell: PaintShell<'_>,
    ) {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return,
        };
        self.paint_element(
            root_id,
            canvas,
            Offset::ZERO,
            focused_node_id,
            image_resource_map,
            shell,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn paint_element(
        &self,
        id: ElementNodeId,
        canvas: &mut dyn Canvas,
        parent_offset: Offset,
        focused_node_id: Option<ElementNodeId>,
        image_resource_map: &ImageResourceMap,
        shell: PaintShell<'_>,
    ) {
        let node = match self.elements.get(&id) {
            Some(n) => n,
            None => return,
        };

        let element = match node.element.as_ref() {
            Some(e) => e,
            None => return,
        };

        let absolute_offset = parent_offset + node.computed_layout.offset;

        let paint_ctx = PaintContext::new(
            self,
            focused_node_id,
            id,
            image_resource_map,
            shell,
        );
        // Flatten: paint fragment children as direct children of this node.
        let children = self.flatten_children(&node.children);
        element.paint(
            canvas,
            absolute_offset,
            &node.computed_layout,
            &children,
            &paint_ctx,
        );
    }

    pub fn hit_test(&self, position: Offset) -> bool {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return false,
        };
        self.hit_test_element(root_id, position)
    }

    pub fn hit_test_path(&self, position: Offset) -> Vec<ElementNodeId> {
        let mut path = Vec::new();
        if let Some(root_id) = self.root_id {
            self.collect_hit_path(root_id, position, &mut path);
        }
        path
    }

    pub fn query_element(&self, key: &[&str]) -> Option<NodeId> {
        let root_id = self.root_id?;
        let mut result = None;
        self.query_element_recursive(NodeId::from(root_id), key, &mut result);
        result
    }

    fn query_element_recursive(
        &self,
        id: NodeId,
        key: &[&str],
        result: &mut Option<NodeId>,
    ) {
        if result.is_some() {
            return;
        }
        let node = match self.elements.get(&ElementNodeId::new(id.as_u64())) {
            Some(n) => n,
            None => return,
        };
        if node
            .query_key
            .as_ref()
            .map(|k| k.iter().map(|s| s.as_str()).eq(key.iter().copied()))
            .unwrap_or(false)
            && node.element.is_some()
        {
            *result = Some(id);
            return;
        }
        for &child in &node.children {
            if self.is_fragment(child) {
                // Check the fragment's own query_key, then recurse its children.
                if let Some(frag) = self.fragments.get(&FragmentNodeId::new(child.as_u64())) {
                    if frag
                        .query_key
                        .as_ref()
                        .map(|k| k.iter().map(|s| s.as_str()).eq(key.iter().copied()))
                        .unwrap_or(false)
                    {
                        *result = Some(child);
                        return;
                    }
                    for &grandchild in &frag.children {
                        if self.is_fragment(grandchild) {
                            self.query_element_recursive_fragment(grandchild, key, result);
                        } else {
                            self.query_element_recursive(grandchild, key, result);
                        }
                        if result.is_some() {
                            return;
                        }
                    }
                }
            } else {
                self.query_element_recursive(child, key, result);
                if result.is_some() {
                    return;
                }
            }
        }
    }

    /// Recurse into a fragment for `query_element` (fragments aren't in `elements`).
    fn query_element_recursive_fragment(
        &self,
        frag_id: NodeId,
        key: &[&str],
        result: &mut Option<NodeId>,
    ) {
        if result.is_some() {
            return;
        }
        let frag = match self.fragments.get(&FragmentNodeId::new(frag_id.as_u64())) {
            Some(f) => f,
            None => return,
        };
        if frag
            .query_key
            .as_ref()
            .map(|k| k.iter().map(|s| s.as_str()).eq(key.iter().copied()))
            .unwrap_or(false)
        {
            *result = Some(frag_id);
            return;
        }
        for &child in &frag.children {
            if self.is_fragment(child) {
                self.query_element_recursive_fragment(child, key, result);
            } else {
                self.query_element_recursive(child, key, result);
            }
            if result.is_some() {
                return;
            }
        }
    }

    fn hit_test_element(&self, id: ElementNodeId, position: Offset) -> bool {
        let node = match self.elements.get(&id) {
            Some(n) => n,
            None => return false,
        };

        let element = match node.element.as_ref() {
            Some(e) => e,
            None => return false,
        };

        let local_position = Offset::new(
            position.x - node.computed_layout.offset.x,
            position.y - node.computed_layout.offset.y,
        );

        if !element.hit_test(local_position, &node.computed_layout) {
            return false;
        }

        // Flatten fragment children and hit-test each in reverse paint order.
        let children = self.flatten_children(&node.children);
        for &child_id in children.iter().rev() {
            if self.hit_test_element(child_id, local_position) {
                return true;
            }
        }

        true
    }

    fn collect_hit_path(
        &self,
        id: ElementNodeId,
        position: Offset,
        path: &mut Vec<ElementNodeId>,
    ) -> bool {
        let node = match self.elements.get(&id) {
            Some(n) => n,
            None => return false,
        };

        let element = match node.element.as_ref() {
            Some(e) => e,
            None => return false,
        };

        let local_position = Offset::new(
            position.x - node.computed_layout.offset.x,
            position.y - node.computed_layout.offset.y,
        );

        if !element.hit_test(local_position, &node.computed_layout) {
            return false;
        }

        // Flatten fragment children and recurse in reverse paint order.
        let children = self.flatten_children(&node.children);
        children
            .iter()
            .rev()
            .any(|&child_id| self.collect_hit_path(child_id, local_position, path));

        path.push(id);

        true
    }

    /// Structured snapshot of one node for the `turDevTool` API.
    ///
    /// Returns `None` if the node or its element is missing. `children` are
    /// bare ids only — callers iterate by invoking `dev_tool_node` per child
    /// id. Lazy traversal keeps payloads small and avoids deep recursion
    /// across the JS bridge.
    pub fn dev_tool_node(&self, id: NodeId) -> Option<DevNodeData> {
        // Real element node:
        if let Some(node) = self.elements.get(&ElementNodeId::new(id.as_u64())) {
            let element = node.element.as_ref()?;
            let relative = node.computed_layout.offset;
            let mut absolute = relative;
            let mut ancestor = node.parent;
            while let Some(pid) = ancestor {
                // Walk through both real elements and fragments. Fragments
                // have zero offset (never laid out) but we must hop to their
                // `.parent` to reach the real ancestor.
                if let Some(p) = self.elements.get(&ElementNodeId::new(pid.as_u64())) {
                    absolute = Offset::new(absolute.x + p.computed_layout.offset.x, absolute.y + p.computed_layout.offset.y);
                    ancestor = p.parent;
                } else if let Some(f) = self.fragments.get(&FragmentNodeId::new(pid.as_u64())) {
                    ancestor = Some(f.parent);
                } else {
                    break;
                }
            }
            return Some(DevNodeData {
                id: node.id.into(),
                name: element.type_name(),
                label: element.trace_label(),
                props: element.trace_props(),
                layout_extra: element.trace_layout_extra(),
                relative: (relative.x, relative.y),
                absolute: (absolute.x, absolute.y),
                size: (node.computed_layout.size.width, node.computed_layout.size.height),
                query_key: node.query_key.clone(),
                children: node.children.to_vec(),
            });
        }
        // Fragment node:
        if let Some(frag) = self.fragments.get(&FragmentNodeId::new(id.as_u64())) {
            let relative = Offset::ZERO;
            let mut absolute = relative;
            let mut ancestor = Some(frag.parent);
            while let Some(pid) = ancestor {
                if let Some(p) = self.elements.get(&ElementNodeId::new(pid.as_u64())) {
                    absolute = Offset::new(absolute.x + p.computed_layout.offset.x, absolute.y + p.computed_layout.offset.y);
                    ancestor = p.parent;
                } else if let Some(pf) = self.fragments.get(&FragmentNodeId::new(pid.as_u64())) {
                    ancestor = Some(pf.parent);
                } else {
                    break;
                }
            }
            return Some(DevNodeData {
                id: frag.id.into(),
                name: frag.type_name(),
                label: frag.trace_label(),
                props: frag.trace_props(),
                layout_extra: vec![],
                relative: (relative.x, relative.y),
                absolute: (absolute.x, absolute.y),
                size: (0.0, 0.0),
                query_key: frag.query_key.clone(),
                children: frag.children.to_vec(),
            });
        }
        None
    }
}

/// Structured snapshot of a single node for the `turDevTool` API.
pub struct DevNodeData {
    pub(crate) id: NodeId,
    pub name: &'static str,
    pub(crate) label: String,
    pub(crate) props: Vec<(&'static str, TraceValue)>,
    pub layout_extra: Vec<(&'static str, TraceValue)>,
    pub(crate) relative: (f64, f64),
    pub(crate) absolute: (f64, f64),
    pub(crate) size: (f64, f64),
    pub(crate) query_key: Option<Vec<String>>,
    pub children: Vec<NodeId>,
}

impl fmt::Debug for NodeTreeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeTreeData")
            .field("elements", &self.elements.len())
            .field("fragments", &self.fragments.len())
            .field("root_id", &self.root_id)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// NodeTree — a clonable handle to the shared node-tree interior
// (`NodeTreeData`, held behind `Rc<RefCell<>>`).
//
// The handle is cheap to clone (Rc bump): controllers and cross-context code
// hold a `NodeTree` and borrow the interior at use time. Layout borrows the
// interior `&mut NodeTreeData` directly (via `borrow_mut`) for the whole
// recursive pass — no competing `Rc<RefCell>` borrow can succeed during
// layout, which is what makes layout-phase mutation (e.g. LazyList remount)
// safe when routed through the layout's own `&mut` borrow.
//
// Ergonomic delegating methods encapsulate the borrow for the common
// operations; reference-returning accessors yield `Ref`/`RefMut` (which deref
// to `&`/`&mut`, so existing call-site chains keep working).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct NodeTree {
    data: Rc<RefCell<NodeTreeData>>,
}

impl NodeTree {
    pub fn new(store: Store) -> Self {
        NodeTree {
            data: Rc::new(RefCell::new(NodeTreeData::new(store))),
        }
    }

    /// Borrow the interior immutably. Prefer the delegating methods below for
    /// single ops; use this when you need to hold the borrow across several
    /// reads (the returned `Ref` derefs to `&NodeTreeData`).
    pub fn borrow(&self) -> Ref<'_, NodeTreeData> {
        self.data.borrow()
    }

    /// Borrow the interior mutably.
    pub fn borrow_mut(&self) -> RefMut<'_, NodeTreeData> {
        self.data.borrow_mut()
    }

    // ----- owned-return delegating methods (borrow internally) -------------

    pub fn alloc_id(&self) -> NodeId {
        self.data.borrow_mut().alloc_id()
    }
    pub fn element_count(&self) -> usize {
        self.data.borrow().element_count()
    }
    pub fn element_ids(&self) -> Vec<ElementNodeId> {
        self.data.borrow().element_ids()
    }
    pub fn insert_element(&self, element: ElementObject) {
        self.data.borrow_mut().insert_element(element);
    }
    pub fn remove_element(&self, id: ElementNodeId) -> Option<ElementObject> {
        self.data.borrow_mut().remove_element(id)
    }
    pub fn root_element_id(&self) -> Option<ElementNodeId> {
        self.data.borrow().root_element_id()
    }
    pub fn set_root_element(&self, id: ElementNodeId) {
        self.data.borrow_mut().set_root_element(id);
    }
    pub fn insert_fragment(&self, host: FragmentHost) {
        self.data.borrow_mut().insert_fragment(host);
    }
    pub fn remove_fragment(&self, id: FragmentNodeId) -> Option<FragmentHost> {
        self.data.borrow_mut().remove_fragment(id)
    }
    pub fn is_fragment(&self, id: NodeId) -> bool {
        self.data.borrow().is_fragment(id)
    }
    pub fn remove_child_entry(&self, parent_id: NodeId, child_id: NodeId) {
        self.data.borrow_mut().remove_child_entry(parent_id, child_id);
    }
    pub fn append_child(&self, parent_id: NodeId, child_id: NodeId) -> bool {
        self.data.borrow_mut().append_child(parent_id, child_id)
    }
    pub fn remove_child(&self, parent_id: NodeId, child_id: NodeId) -> bool {
        self.data.borrow_mut().remove_child(parent_id, child_id)
    }
    pub fn insert_before(
        &self,
        parent_id: ElementNodeId,
        child_id: NodeId,
        ref_id: ElementNodeId,
    ) -> bool {
        self.data
            .borrow_mut()
            .insert_before(parent_id, child_id, ref_id)
    }
    pub fn children_of_element(&self, id: ElementNodeId) -> Vec<ElementNodeId> {
        self.data.borrow().children_of_element(id)
    }
    pub fn raw_children_of_element(&self, id: ElementNodeId) -> Vec<NodeId> {
        self.data.borrow().raw_children_of_element(id)
    }
    pub fn parent_of_element(&self, id: ElementNodeId) -> Option<NodeId> {
        self.data.borrow().parent_of_element(id)
    }
    pub fn mark_dirty(&self, id: NodeId) {
        self.data.borrow_mut().mark_dirty(id);
    }
    pub fn destroy_subtree(&self, id: ElementNodeId) {
        self.data.borrow_mut().destroy_subtree(id);
    }
    pub fn destroy_child(&self, id: NodeId) {
        self.data.borrow_mut().destroy_child(id);
    }
    pub fn destroy_fragment(&self, id: FragmentNodeId) {
        self.data.borrow_mut().destroy_fragment(id);
    }
    pub fn move_child_before(&self, parent: ElementNodeId, child: NodeId, ref_child: NodeId) {
        self.data.borrow_mut().move_child_before(parent, child, ref_child);
    }
    pub fn mark_root_dirty(&self) {
        self.data.borrow_mut().mark_root_dirty();
    }
    pub fn has_dirty_layout(&self) -> bool {
        self.data.borrow().has_dirty_layout()
    }
    pub fn hit_test(&self, position: Offset) -> bool {
        self.data.borrow().hit_test(position)
    }
    pub fn hit_test_path(&self, position: Offset) -> Vec<ElementNodeId> {
        self.data.borrow().hit_test_path(position)
    }
    pub fn query_element(&self, key: &[&str]) -> Option<NodeId> {
        self.data.borrow().query_element(key)
    }
    pub fn dev_tool_node(&self, id: NodeId) -> Option<DevNodeData> {
        self.data.borrow().dev_tool_node(id)
    }

    // ----- reference-returning accessors (Ref / RefMut) --------------------

    pub fn get_element(&self, id: ElementNodeId) -> Option<Ref<'_, ElementObject>> {
        Ref::filter_map(self.data.borrow(), |d| d.elements.get(&id)).ok()
    }
    pub fn get_element_mut(&self, id: ElementNodeId) -> Option<RefMut<'_, ElementObject>> {
        RefMut::filter_map(self.data.borrow_mut(), |d| d.elements.get_mut(&id)).ok()
    }
    pub fn root_element(&self) -> Option<Ref<'_, ElementObject>> {
        Ref::filter_map(self.data.borrow(), |d| {
            d.root_id.and_then(|id| d.elements.get(&id))
        })
        .ok()
    }
    pub fn get_fragment(&self, id: FragmentNodeId) -> Option<Ref<'_, FragmentHost>> {
        Ref::filter_map(self.data.borrow(), |d| d.fragments.get(&id)).ok()
    }
    pub fn get_fragment_mut(&self, id: FragmentNodeId) -> Option<RefMut<'_, FragmentHost>> {
        RefMut::filter_map(self.data.borrow_mut(), |d| d.fragments.get_mut(&id)).ok()
    }

    // ----- sustained-borrow ops (borrow spans the whole recursive call) ----

    #[allow(clippy::too_many_arguments)]
    pub fn compute_layout(
        &self,
        constraints: &Constraints,
        font_manager: &mut FontManager,
        text_layout_cx: &mut ParleyLayoutContext<[u8; 4]>,
        image_resource_map: &ImageResourceMap,
        node_tree: NodeTree,
        mutation_queue: std::rc::Rc<std::cell::RefCell<crate::core::edgy::mutation::PendingMutationInvocationQueue>>,
        dirty: std::rc::Rc<std::cell::Cell<bool>>,
        boa: &mut boa_engine::Context,
    ) -> Size {
        self.data.borrow_mut().compute_layout(
            constraints,
            font_manager,
            text_layout_cx,
            image_resource_map,
            node_tree,
            mutation_queue,
            dirty,
            boa,
        )
    }

    pub fn paint(
        &self,
        canvas: &mut dyn Canvas,
        focused_node_id: Option<ElementNodeId>,
        image_resource_map: &ImageResourceMap,
        shell: PaintShell<'_>,
    ) {
        self.data
            .borrow()
            .paint(canvas, focused_node_id, image_resource_map, shell);
    }
}

impl fmt::Debug for NodeTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.data.try_borrow() {
            Ok(d) => fmt::Debug::fmt(&d, f),
            Err(_) => f.debug_struct("NodeTree").field("borrowed", &true).finish(),
        }
    }
}
