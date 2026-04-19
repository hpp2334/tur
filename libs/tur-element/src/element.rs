use std::collections::HashMap;
use std::str::FromStr;

use tur_shared::ComputedLayout;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementKind {
    Flex,
    FlexItem,
    Stack,
    Positioned,
    Container,
    Text,
}

impl FromStr for ElementKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tur_flex" => Ok(ElementKind::Flex),
            "tur_flex_item" => Ok(ElementKind::FlexItem),
            "tur_stack" => Ok(ElementKind::Stack),
            "tur_positioned" => Ok(ElementKind::Positioned),
            "tur_container" => Ok(ElementKind::Container),
            "tur_text" => Ok(ElementKind::Text),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum PropValue {
    String(String),
    Number(f64),
    Bool(bool),
}

impl PropValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            PropValue::Number(n) => Some(*n),
            PropValue::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PropValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ElementNode {
    pub id: ElementNodeId,
    pub kind: ElementKind,
    pub props: HashMap<String, PropValue>,
    pub children: Vec<ElementNodeId>,
    pub parent: Option<ElementNodeId>,
    pub computed_layout: Option<ComputedLayout>,
}

impl ElementNode {
    pub fn new(id: ElementNodeId, kind: ElementKind) -> Self {
        ElementNode {
            id,
            kind,
            props: HashMap::new(),
            children: Vec::new(),
            parent: None,
            computed_layout: None,
        }
    }

    pub fn set_prop(&mut self, key: String, value: PropValue) {
        self.props.insert(key, value);
    }

    pub fn get_prop(&self, key: &str) -> Option<&PropValue> {
        self.props.get(key)
    }

    pub fn prop_str(&self, key: &str) -> Option<&str> {
        self.props.get(key).and_then(|v| v.as_str())
    }

    pub fn prop_f64(&self, key: &str) -> Option<f64> {
        self.props.get(key).and_then(|v| v.as_f64())
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
