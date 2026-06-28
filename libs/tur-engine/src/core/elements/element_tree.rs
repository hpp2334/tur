use std::collections::HashMap;
use std::fmt;

use parley::LayoutContext as ParleyLayoutContext;
use tur_shared::{Constraints, Offset, Size};

use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::elements::{ElementObject, FragmentHost, TraceValue};
use crate::core::fonts::FontManager;
use crate::core::layout::{LayoutContext, SubscribeCx};
use crate::core::reactive::{ReactiveReadStore, ReactiveReadJsContext, Store, SubscriberId};
use crate::core::render::{Canvas, PaintContext};
use crate::core::shell::PaintShell;
use crate::core::resource::ResourceMap;

pub struct ElementTree {
    pub(crate) elements: HashMap<ElementNodeId, ElementObject>,
    /// Control-flow primitives (Each / Condition / Switch). Keyed by id (same
    /// counter as `elements`). Fragments have no `AnyElement` and are never laid
    /// out / painted directly — `flatten_children` splices their children into
    /// the enclosing flex's layout.
    pub(crate) fragments: HashMap<FragmentNodeId, FragmentHost>,
    root_id: Option<ElementNodeId>,
    next_id: u64,
    store: Store,
    /// Cached read-only reactive face; the layout driver wraps this in a
    /// [`ReactiveReadJsContext`] (with a `Context` borrow) so layout can only
    /// read atoms, never `set` / mutate.
    read_face: ReactiveReadStore,
}

impl ElementTree {
    pub fn new(store: Store) -> Self {
        let read_face = store.read_only();
        ElementTree {
            elements: HashMap::new(),
            fragments: HashMap::new(),
            root_id: None,
            next_id: 1,
            store,
            read_face,
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

    pub fn insert_element(&mut self, element: ElementObject) {
        if self.root_id.is_none() {
            self.root_id = Some(element.id);
        }
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
        parent_id: NodeId,
        child_id: NodeId,
        ref_id: NodeId,
    ) -> bool {
        if !self.elements.contains_key(&ElementNodeId::new(parent_id.as_u64()))
            || (!self.elements.contains_key(&ElementNodeId::new(child_id.as_u64()))
                && !self.fragments.contains_key(&FragmentNodeId::new(child_id.as_u64())))
            || !self.elements.contains_key(&ElementNodeId::new(ref_id.as_u64()))
        {
            return false;
        }
        // Set the child's parent pointer.
        if let Some(c) = self.elements.get_mut(&ElementNodeId::new(child_id.as_u64())) {
            c.parent = Some(parent_id);
        } else if let Some(f) = self.fragments.get_mut(&FragmentNodeId::new(child_id.as_u64())) {
            f.parent = parent_id;
        }
        let insert_fn = |children: &mut Vec<NodeId>| {
            if let Some(pos) = children.iter().position(|c| *c == ref_id) {
                children.insert(pos, child_id);
            } else {
                children.push(child_id);
            }
        };
        if let Some(node) = self.elements.get_mut(&ElementNodeId::new(parent_id.as_u64())) {
            insert_fn(&mut node.children);
        } else if let Some(frag) = self.fragments.get_mut(&FragmentNodeId::new(parent_id.as_u64())) {
            insert_fn(&mut frag.children);
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

    pub fn first_child_of_element(&self, id: ElementNodeId) -> Option<ElementNodeId> {
        self.elements
            .get(&id)
            .and_then(|n| n.children.first().copied())
            .map(|c| ElementNodeId::new(c.as_u64()))
    }

    pub fn next_sibling_of_element(&self, id: ElementNodeId) -> Option<ElementNodeId> {
        let parent_id = self.elements.get(&id).and_then(|n| n.parent)?;
        let parent_children: &[NodeId] = self
            .elements
            .get(&ElementNodeId::new(parent_id.as_u64()))
            .map(|n| &n.children[..])
            .or_else(|| {
                self.fragments
                    .get(&FragmentNodeId::new(parent_id.as_u64()))
                    .map(|f| &f.children[..])
            })?;
        let pos = parent_children.iter().position(|c| *c == NodeId::from(id))?;
        parent_children
            .get(pos + 1)
            .copied()
            .map(|c| ElementNodeId::new(c.as_u64()))
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

    pub fn compute_layout(
        &mut self,
        constraints: &Constraints,
        font_manager: &mut FontManager,
        text_layout_cx: &mut ParleyLayoutContext<[u8; 4]>,
        resource_map: &ResourceMap,
        boa: &mut boa_engine::Context,
    ) -> Size {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return constraints.constrain(Size::ZERO),
        };

        let mut js = ReactiveReadJsContext::new(self.read_face.clone(), boa);
        self.layout(root_id, constraints, font_manager, text_layout_cx, resource_map, &mut js)
    }

    pub(crate) fn layout<'a, 'js>(
        &'a mut self,
        id: ElementNodeId,
        constraints: &Constraints,
        font_manager: &'a mut FontManager,
        text_layout_cx: &'a mut ParleyLayoutContext<[u8; 4]>,
        resource_map: &'a ResourceMap,
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

        let mut cx = LayoutContext::new(self, id, font_manager, text_layout_cx, resource_map, js);
        // `perform_layout` measures the children (recursively laying each out
        // via `cx.layout_child`), computes this node's size, and assigns each
        // child's offset — all in one pass.
        let size = element.perform_layout(constraints, &children, &mut cx);

        // Explicit subscribe phase: re-declare this node's reactive deps so a
        // future reactive flush can mark it dirty. The `SubscribeCx` swap
        // (on drop) replaces the node's prior subscriptions.
        let sub_index = cx.tree.store.subscriber_index();
        let mut sub_cx = SubscribeCx::new(sub_index, SubscriberId::new(id.as_u64()));
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
        focused_node_id: Option<NodeId>,
        resource_map: &ResourceMap,
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
            resource_map,
            shell,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_element(
        &self,
        id: ElementNodeId,
        canvas: &mut dyn Canvas,
        parent_offset: Offset,
        focused_node_id: Option<NodeId>,
        resource_map: &ResourceMap,
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
            resource_map,
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

    pub fn hit_test_path(&self, position: Offset) -> Vec<NodeId> {
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
        path: &mut Vec<NodeId>,
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

        path.push(NodeId::from(id));

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
                id: NodeId::new(node.id.as_u64()),
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
                id: NodeId::new(frag.id.as_u64()),
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
    pub id: NodeId,
    pub name: &'static str,
    pub label: String,
    pub props: Vec<(&'static str, TraceValue)>,
    pub layout_extra: Vec<(&'static str, TraceValue)>,
    pub relative: (f64, f64),
    pub absolute: (f64, f64),
    pub size: (f64, f64),
    pub query_key: Option<Vec<String>>,
    pub children: Vec<NodeId>,
}

impl fmt::Debug for ElementTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElementTree")
            .field("elements", &self.elements.len())
            .field("fragments", &self.fragments.len())
            .field("root_id", &self.root_id)
            .finish()
    }
}
