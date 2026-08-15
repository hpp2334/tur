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

/// Identifies one view root (logical mount slot) inside an engine instance.
/// Assigned at build time (see `TurAppBuilder::view_root`) and stable for the
/// instance's lifetime. Carried by routed `PlatformEvent`s (resize / pointer /
/// wheel), render batches (`MainMsg::RenderCommands`), and cursor changes so
/// the engine can address the correct root's tree / screen / render target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewRootId(u32);

impl ViewRootId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for ViewRootId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A node id: the owning view root plus a counter unique **within that
/// root's tree**. The root field is what makes node ids unique instance-wide
/// (each tree allocates from its own counter starting at 1), so the owning
/// tree of any node id can be resolved in O(1) via `root()` — no tree scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId {
    root: ViewRootId,
    node: u64,
}

impl NodeId {
    pub const fn new(root: ViewRootId, node: u64) -> Self {
        Self { root, node }
    }

    /// The view root whose tree owns (or would own) this node.
    pub const fn root(self) -> ViewRootId {
        self.root
    }

    /// The per-tree node counter (unique within the root's tree only).
    pub const fn node(self) -> u64 {
        self.node
    }

    /// Re-wrap as an element id (guaranteed-by-construction: callers only
    /// invoke this when the node is known to be an element). Preserves the
    /// owning root.
    pub const fn as_element_id(self) -> ElementNodeId {
        ElementNodeId {
            root: self.root,
            node: self.node,
        }
    }

    /// Re-wrap as a fragment id (guaranteed-by-construction). Preserves the
    /// owning root.
    pub const fn as_fragment_id(self) -> FragmentNodeId {
        FragmentNodeId {
            root: self.root,
            node: self.node,
        }
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "r{}:n{}", self.root.0, self.node)
    }
}

/// A node id that is guaranteed to reference a **real element** in
/// `ElementTree::elements` (never a fragment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementNodeId {
    root: ViewRootId,
    node: u64,
}

impl ElementNodeId {
    pub const fn new(root: ViewRootId, node: u64) -> Self {
        Self { root, node }
    }

    /// The view root whose tree owns this element.
    pub const fn root(self) -> ViewRootId {
        self.root
    }

    /// The per-tree node counter (unique within the root's tree only).
    pub const fn node(self) -> u64 {
        self.node
    }
}

impl From<ElementNodeId> for NodeId {
    fn from(id: ElementNodeId) -> Self {
        NodeId {
            root: id.root,
            node: id.node,
        }
    }
}

impl std::fmt::Display for ElementNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "r{}:n{}", self.root.0, self.node)
    }
}

/// A node id that is guaranteed to reference a **fragment** in
/// `ElementTree::fragments` (Each / Condition / Switch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FragmentNodeId {
    root: ViewRootId,
    node: u64,
}

impl FragmentNodeId {
    pub const fn new(root: ViewRootId, node: u64) -> Self {
        Self { root, node }
    }

    /// The view root whose tree owns this fragment.
    pub const fn root(self) -> ViewRootId {
        self.root
    }

    /// The per-tree node counter (unique within the root's tree only).
    pub const fn node(self) -> u64 {
        self.node
    }
}

impl From<FragmentNodeId> for NodeId {
    fn from(id: FragmentNodeId) -> Self {
        NodeId {
            root: id.root,
            node: id.node,
        }
    }
}

impl std::fmt::Display for FragmentNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "r{}:n{}", self.root.0, self.node)
    }
}
