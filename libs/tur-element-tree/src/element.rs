use std::collections::HashMap;

use tur_trait::{DynElement, ElementKind, ElementTreeProvider, RenderObject};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementNodeId(u64);

impl ElementNodeId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

pub trait Element: Send + Sync + Clone + 'static {
    type TypedRenderObject: RenderObject;

    fn to_render_object(&self) -> Self::TypedRenderObject;
    fn kind(&self) -> ElementKind;
    fn name(&self) -> &'static str;
}

pub struct ElementNode {
    pub id: ElementNodeId,
    pub element: Box<dyn DynElement>,
    pub children: Vec<ElementNodeId>,
    pub parent: Option<ElementNodeId>,
}

impl Clone for ElementNode {
    fn clone(&self) -> Self {
        ElementNode {
            id: self.id,
            element: self.element.clone_box(),
            children: self.children.clone(),
            parent: self.parent,
        }
    }
}

impl std::fmt::Debug for ElementNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ElementNode")
            .field("id", &self.id)
            .field("element", &self.element.name())
            .field("children", &self.children)
            .field("parent", &self.parent)
            .finish()
    }
}

impl ElementNode {
    pub fn new(id: ElementNodeId, element: Box<dyn DynElement>) -> Self {
        ElementNode {
            id,
            element,
            children: Vec::new(),
            parent: None,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ElementTree {
    nodes: HashMap<ElementNodeId, ElementNode>,
    root_id: Option<ElementNodeId>,
}

impl ElementTree {
    pub fn new() -> Self {
        ElementTree {
            nodes: HashMap::new(),
            root_id: None,
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn insert(&mut self, node: ElementNode) {
        if self.root_id.is_none() {
            self.root_id = Some(node.id);
        }
        self.nodes.insert(node.id, node);
    }

    pub fn get(&self, id: ElementNodeId) -> Option<&ElementNode> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: ElementNodeId) -> Option<&mut ElementNode> {
        self.nodes.get_mut(&id)
    }

    pub fn remove(&mut self, id: ElementNodeId) -> Option<ElementNode> {
        let node = self.nodes.remove(&id)?;
        if self.root_id == Some(id) {
            self.root_id = None;
        }
        Some(node)
    }

    pub fn root_id(&self) -> Option<ElementNodeId> {
        self.root_id
    }

    pub fn root(&self) -> Option<&ElementNode> {
        self.root_id.and_then(|id| self.nodes.get(&id))
    }

    pub fn root_mut(&mut self) -> Option<&mut ElementNode> {
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
}

impl ElementTreeProvider for ElementTree {
    fn root_id(&self) -> Option<u64> {
        self.root_id.map(|id| id.as_u64())
    }

    fn children_of(&self, id: u64) -> Vec<u64> {
        self.children_of(ElementNodeId::new(id))
            .iter()
            .map(|c| c.as_u64())
            .collect()
    }

    fn element_for(&self, id: u64) -> &dyn DynElement {
        self.get(ElementNodeId::new(id))
            .map(|n| n.element.as_ref())
            .expect("node not found")
    }
}
