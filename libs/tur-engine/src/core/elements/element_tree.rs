use std::collections::{HashMap, HashSet};
use std::cell::RefCell;
use std::rc::Rc;

use parley::LayoutContext as ParleyLayoutContext;
use tur_shared::{Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::elements::{ElementObject, TraceValue};
use crate::core::fonts::FontManager;
use crate::core::layout::LayoutContext;
use crate::core::reactive::{AtomId, Store};
use crate::core::render::{Canvas, PaintContext};
use crate::core::resource::ResourceMap;

/// Atom→node dependency tracker.  Populated automatically during layout
/// when `LayoutContext::read_val` resolves a reactive `Val<T>`.  After a
/// reactive flush, `dirty_nodes` returns the set of nodes that read any
/// of the dirty atoms and therefore need re-layout.
#[derive(Default)]
pub struct DepTracker {
    atom_to_nodes: RefCell<HashMap<AtomId, HashSet<ElementNodeId>>>,
    node_to_atoms: RefCell<HashMap<ElementNodeId, HashSet<AtomId>>>,
}

impl std::fmt::Debug for DepTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DepTracker")
            .field("atoms", &self.atom_to_nodes.borrow().len())
            .field("nodes", &self.node_to_atoms.borrow().len())
            .finish()
    }
}

impl DepTracker {
    pub fn track(&self, atom: AtomId, node: ElementNodeId) {
        self.atom_to_nodes.borrow_mut().entry(atom).or_default().insert(node);
        self.node_to_atoms.borrow_mut().entry(node).or_default().insert(atom);
    }

    /// Clear all outgoing deps for `node` (called before re-layout so stale
    /// entries from the previous pass don't linger).
    pub fn clear_node(&self, node: ElementNodeId) {
        if let Some(atoms) = self.node_to_atoms.borrow_mut().remove(&node) {
            for atom in atoms {
                let should_remove = {
                    let mut map = self.atom_to_nodes.borrow_mut();
                    if let Some(nodes) = map.get_mut(&atom) {
                        nodes.remove(&node);
                        nodes.is_empty()
                    } else {
                        false
                    }
                };
                if should_remove {
                    self.atom_to_nodes.borrow_mut().remove(&atom);
                }
            }
        }
    }

    /// Return nodes that depend on any of the given dirty atoms.
    pub fn dirty_nodes(&self, dirty: &HashSet<AtomId>) -> HashSet<ElementNodeId> {
        let mut out = HashSet::new();
        for atom in dirty {
            if let Some(nodes) = self.atom_to_nodes.borrow().get(atom) {
                out.extend(nodes);
            }
        }
        out
    }
}

#[derive(Debug)]
pub struct ElementTree {
    pub(crate) nodes: HashMap<ElementNodeId, ElementObject>,
    root_id: Option<ElementNodeId>,
    next_id: u64,
    /// Reactive store (set via `set_store`).  Enables `LayoutContext` and
    /// `PaintContext` to resolve `Val<T>` values on demand.
    pub(crate) store: Option<Rc<RefCell<Store>>>,
    /// Atom→node dependency map for fine-grained dirty propagation.
    pub(crate) dep_tracker: DepTracker,
}

impl Default for ElementTree {
    fn default() -> Self {
        Self::new()
    }
}

impl ElementTree {
    pub fn new() -> Self {
        ElementTree {
            nodes: HashMap::new(),
            root_id: None,
            next_id: 1,
            store: None,
            dep_tracker: DepTracker::default(),
        }
    }

    /// Attach the reactive store so layout/paint can resolve `Val<T>`.
    pub fn set_store(&mut self, store: Rc<RefCell<Store>>) {
        self.store = Some(store);
    }

    pub fn dep_tracker(&self) -> &DepTracker {
        &self.dep_tracker
    }

    pub fn alloc_id(&mut self) -> ElementNodeId {
        let id = self.next_id;
        self.next_id += 1;
        ElementNodeId::new(id)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn insert(&mut self, node: ElementObject) {
        if self.root_id.is_none() {
            self.root_id = Some(node.id);
        }
        self.nodes.insert(node.id, node);
    }

    pub fn get(&self, id: ElementNodeId) -> Option<&ElementObject> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: ElementNodeId) -> Option<&mut ElementObject> {
        self.nodes.get_mut(&id)
    }

    pub fn remove(&mut self, id: ElementNodeId) -> Option<ElementObject> {
        let node = self.nodes.remove(&id)?;
        if self.root_id == Some(id) {
            self.root_id = None;
        }
        Some(node)
    }

    pub fn root_id(&self) -> Option<ElementNodeId> {
        self.root_id
    }

    pub fn root(&self) -> Option<&ElementObject> {
        self.root_id.and_then(|id| self.nodes.get(&id))
    }

    pub fn root_mut(&mut self) -> Option<&mut ElementObject> {
        self.root_id.and_then(|id| self.nodes.get_mut(&id))
    }

    pub fn set_root(&mut self, id: ElementNodeId) {
        self.root_id = Some(id);
    }

    pub fn append_child(&mut self, parent_id: ElementNodeId, child_id: ElementNodeId) -> bool {
        if !self.nodes.contains_key(&parent_id) || !self.nodes.contains_key(&child_id) {
            return false;
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = Some(parent_id);
        }
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.push(child_id);
        }
        true
    }

    pub fn remove_child(&mut self, parent_id: ElementNodeId, child_id: ElementNodeId) -> bool {
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            if let Some(pos) = parent.children.iter().position(|&id| id == child_id) {
                parent.children.remove(pos);
            }
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = None;
        }
        true
    }

    pub fn insert_before(
        &mut self,
        parent_id: ElementNodeId,
        child_id: ElementNodeId,
        ref_id: ElementNodeId,
    ) -> bool {
        if !self.nodes.contains_key(&parent_id)
            || !self.nodes.contains_key(&child_id)
            || !self.nodes.contains_key(&ref_id)
        {
            return false;
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = Some(parent_id);
        }
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            if let Some(pos) = parent.children.iter().position(|&id| id == ref_id) {
                parent.children.insert(pos, child_id);
            } else {
                parent.children.push(child_id);
            }
        }
        true
    }

    pub fn children_of(&self, id: ElementNodeId) -> Vec<ElementNodeId> {
        self.nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default()
    }

    pub fn parent_of(&self, id: ElementNodeId) -> Option<ElementNodeId> {
        self.nodes.get(&id).and_then(|n| n.parent)
    }

    pub fn first_child_of(&self, id: ElementNodeId) -> Option<ElementNodeId> {
        self.nodes
            .get(&id)
            .and_then(|n| n.children.first().copied())
    }

    pub fn next_sibling_of(&self, id: ElementNodeId) -> Option<ElementNodeId> {
        let parent_id = self.nodes.get(&id).and_then(|n| n.parent)?;
        let parent = self.nodes.get(&parent_id)?;
        let pos = parent.children.iter().position(|&c| c == id)?;
        parent.children.get(pos + 1).copied()
    }

    pub fn mark_dirty(&mut self, id: ElementNodeId) {
        let mut path = Vec::new();
        {
            let mut current = Some(id);
            while let Some(cid) = current {
                let node = match self.nodes.get(&cid) {
                    Some(n) => n,
                    None => break,
                };
                if node.dirty_layout {
                    break;
                }
                path.push(cid);
                current = node.parent;
            }
        }
        for cid in path {
            if let Some(node) = self.nodes.get_mut(&cid) {
                node.dirty_layout = true;
                node.dirty_paint = true;
            }
        }
    }

    pub fn mark_dirty_paint(&mut self, id: ElementNodeId) {
        let mut path = Vec::new();
        {
            let mut current = Some(id);
            while let Some(cid) = current {
                let node = match self.nodes.get(&cid) {
                    Some(n) => n,
                    None => break,
                };
                if node.dirty_paint {
                    break;
                }
                path.push(cid);
                current = node.parent;
            }
        }
        for cid in path {
            if let Some(node) = self.nodes.get_mut(&cid) {
                node.dirty_paint = true;
            }
        }
    }

    pub fn mark_root_dirty(&mut self) {
        // A change to the viewport size can alter the constraints of every
        // node, so the whole tree must be re-laid-out. (`layout_size`
        // short-circuits on the per-node `dirty_layout` flag, so marking only
        // the root would leave the rest of the tree with stale sizes.) This is
        // only invoked on resize.
        for node in self.nodes.values_mut() {
            node.dirty_layout = true;
            node.dirty_paint = true;
        }
    }

    pub fn has_dirty_layout(&self) -> bool {
        self.nodes.values().any(|n| n.dirty_layout)
    }

    pub fn compute_layout(
        &mut self,
        constraints: &Constraints,
        font_manager: &mut FontManager,
        text_layout_cx: &mut ParleyLayoutContext<[u8; 4]>,
        resource_map: &ResourceMap,
    ) -> Size {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return constraints.constrain(Size::ZERO),
        };

        let size = self.layout_size(root_id, constraints, font_manager, text_layout_cx, resource_map);
        self.layout_position(root_id, font_manager, text_layout_cx, resource_map);

        size
    }

    pub(crate) fn layout_size(
        &mut self,
        id: ElementNodeId,
        constraints: &Constraints,
        font_manager: &mut FontManager,
        text_layout_cx: &mut ParleyLayoutContext<[u8; 4]>,
        resource_map: &ResourceMap,
    ) -> Size {
        let (is_dirty, constraints_changed) = self
            .nodes
            .get(&id)
            .map(|n| (n.dirty_layout, n.last_constraints != Some(*constraints)))
            .unwrap_or((false, true));
        if !is_dirty && !constraints_changed {
            return self.nodes.get(&id)
                .map(|n| n.computed_layout.size)
                .unwrap_or(Size::ZERO);
        }

        // Clear stale atom deps for this node — they'll be re-registered
        // during layout as the element calls `cx.read_val`.
        self.dep_tracker.clear_node(id);

        let children = self
            .nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();

        let mut element = self
            .nodes
            .get_mut(&id)
            .and_then(|n| n.element.take())
            .expect("element missing during layout_size");

        let mut cx = LayoutContext::new(self, id, font_manager, text_layout_cx, resource_map);
        let size = element.perform_layout_size(constraints, &children, &mut cx);

        let constrained = constraints.constrain(size);
        let node = cx.tree.nodes.get_mut(&id).unwrap();
        node.element = Some(element);
        node.computed_layout.size = constrained;
        node.last_constraints = Some(*constraints);
        node.dirty_layout = false;
        constrained
    }

    fn layout_position(
        &mut self,
        id: ElementNodeId,
        font_manager: &mut FontManager,
        text_layout_cx: &mut ParleyLayoutContext<[u8; 4]>,
        resource_map: &ResourceMap,
    ) {
        let children = self
            .nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();

        {
            let mut element = self
                .nodes
                .get_mut(&id)
                .and_then(|n| n.element.take())
                .expect("element missing during layout_position");

            let mut cx = LayoutContext::new(self, id, font_manager, text_layout_cx, resource_map);
            element.perform_layout_position(&children, &mut cx);

            cx.tree.nodes.get_mut(&id).unwrap().element = Some(element);
        }

        for child_id in children {
            self.layout_position(child_id, font_manager, text_layout_cx, resource_map);
        }
    }

    pub fn paint(&self, canvas: &mut dyn Canvas, focused_node_id: Option<ElementNodeId>, resource_map: &ResourceMap) {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return,
        };
        self.paint_node(root_id, canvas, Offset::ZERO, focused_node_id, resource_map);
    }

    pub(crate) fn paint_node(
        &self,
        id: ElementNodeId,
        canvas: &mut dyn Canvas,
        parent_offset: Offset,
        focused_node_id: Option<ElementNodeId>,
        resource_map: &ResourceMap,
    ) {
        let node = match self.nodes.get(&id) {
            Some(n) => n,
            None => return,
        };

        let element = match node.element.as_ref() {
            Some(e) => e,
            None => return,
        };

        let absolute_offset = parent_offset + node.computed_layout.offset;

        let paint_ctx = PaintContext::new(self, focused_node_id, id, resource_map);
        element.paint(
            canvas,
            absolute_offset,
            &node.computed_layout,
            &node.children,
            &paint_ctx,
        );
    }

    pub fn hit_test(&self, position: Offset) -> bool {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return false,
        };
        self.hit_test_node(root_id, position)
    }

    pub fn hit_test_path(&self, position: Offset) -> Vec<ElementNodeId> {
        let mut path = Vec::new();
        if let Some(root_id) = self.root_id {
            self.collect_hit_path(root_id, position, &mut path);
        }
        path
    }

    pub fn query_element(&self, key: &[&str]) -> Option<ElementNodeId> {
        let root_id = self.root_id?;
        let mut result = None;
        self.query_element_recursive(root_id, key, &mut result);
        result
    }

    fn query_element_recursive(
        &self,
        id: ElementNodeId,
        key: &[&str],
        result: &mut Option<ElementNodeId>,
    ) {
        if result.is_some() {
            return;
        }
        let node = match self.nodes.get(&id) {
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
        for &child_id in &node.children {
            self.query_element_recursive(child_id, key, result);
            if result.is_some() {
                return;
            }
        }
    }

    fn hit_test_node(&self, id: ElementNodeId, position: Offset) -> bool {
        let node = match self.nodes.get(&id) {
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

        for &child_id in node.children.iter().rev() {
            if self.hit_test_node(child_id, local_position) {
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
        let node = match self.nodes.get(&id) {
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

        node.children
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
    pub fn dev_tool_node(&self, id: ElementNodeId) -> Option<DevNodeData> {
        let node = self.nodes.get(&id)?;
        let element = node.element.as_ref()?;
        let relative = node.computed_layout.offset;
        // Absolute offset = sum of this node's offset plus every ancestor's.
        let mut absolute = relative;
        let mut ancestor = node.parent;
        while let Some(pid) = ancestor {
            let p = self.nodes.get(&pid)?;
            absolute = Offset::new(absolute.x + p.computed_layout.offset.x, absolute.y + p.computed_layout.offset.y);
            ancestor = p.parent;
        }
        Some(DevNodeData {
            id: node.id,
            name: element.type_name(),
            label: element.trace_label(),
            props: element.trace_props(),
            layout_extra: element.trace_layout_extra(),
            relative: (relative.x, relative.y),
            absolute: (absolute.x, absolute.y),
            size: (node.computed_layout.size.width, node.computed_layout.size.height),
            query_key: node.query_key.clone(),
            children: node.children.clone(),
        })
    }
}

/// Structured snapshot of a single node for the `turDevTool` API.
pub struct DevNodeData {
    pub id: ElementNodeId,
    pub name: &'static str,
    pub label: String,
    pub props: Vec<(&'static str, TraceValue)>,
    pub layout_extra: Vec<(&'static str, TraceValue)>,
    pub relative: (f64, f64),
    pub absolute: (f64, f64),
    pub size: (f64, f64),
    pub query_key: Option<Vec<String>>,
    pub children: Vec<ElementNodeId>,
}
