use std::rc::Rc;

use crate::core::element::NodeId;
use crate::core::elements::ElementTrace;
use crate::core::view::{Lifecycle, View, ViewCx};

// ---------------------------------------------------------------------------
// FragmentView — a transparent multi-child container. Children are built
// directly under a single FragmentElement node (which renders nothing and sizes to
// the union of its children).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FragmentView {
    children: Vec<Rc<dyn View>>,
    #[allow(dead_code)]
    query_key: Option<Vec<String>>,
}

impl FragmentView {
    /// Rut-rail constructor (`core::rut_runtime`): a transparent group.
    pub fn new_rut(children: Vec<Rc<dyn View>>) -> Self {
        Self { children, query_key: None }
    }

    /// Append one child (the `frag_child` row).
    pub(crate) fn push_child(&mut self, child: Rc<dyn View>) {
        self.children.push(child);
    }
}

impl View for FragmentView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        // FragmentElement is truly transparent — no node is created. Children are
        // built directly under the parent. This matches React FragmentElement
        // semantics and keeps the tree flat for tests that navigate
        // root.children directly.
        for child_spec in &self.children {
            child_spec.build(cx, parent);
        }
        parent
    }
}

// ---------------------------------------------------------------------------
// FragmentElement element — only used if a FragmentElement node is ever created directly
// (normally FragmentElement is transparent and creates no node). Kept for potential
// future use.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub struct FragmentElement {
    pub(crate) view: FragmentView,
}

impl crate::core::layout::ElementSubscribe for FragmentElement {}

impl Lifecycle for FragmentElement {}

impl ElementTrace for FragmentElement {}
