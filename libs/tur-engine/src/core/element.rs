use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElementKind(Arc<str>);

impl ElementKind {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
}

impl std::fmt::Display for ElementKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(u64);

impl NodeId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A node id that is guaranteed to reference a **real element** in
/// `ElementTree::elements` (never a fragment).
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

impl From<ElementNodeId> for NodeId {
    fn from(id: ElementNodeId) -> Self {
        NodeId(id.0)
    }
}

impl std::fmt::Display for ElementNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A node id that is guaranteed to reference a **fragment** in
/// `ElementTree::fragments` (Each / Condition / Switch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FragmentNodeId(u64);

impl FragmentNodeId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl From<FragmentNodeId> for NodeId {
    fn from(id: FragmentNodeId) -> Self {
        NodeId(id.0)
    }
}

impl std::fmt::Display for FragmentNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

