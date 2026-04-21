use std::fmt;

use tur_shared::ComputedLayout;

use crate::core::element::ElementNodeId;
use crate::core::elements::AnyElement;

pub struct ElementNode {
    pub id: ElementNodeId,
    pub element: Option<AnyElement>,
    pub children: Vec<ElementNodeId>,
    pub parent: Option<ElementNodeId>,
    pub computed_layout: ComputedLayout,
    pub query_key: Option<Vec<String>>,
}

impl fmt::Debug for ElementNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElementNode")
            .field("id", &self.id)
            .field("kind", &self.element.as_ref().map(|e| e.kind()))
            .field("children", &self.children)
            .field("parent", &self.parent)
            .finish()
    }
}

impl ElementNode {
    pub fn new(id: ElementNodeId, element: AnyElement) -> Self {
        ElementNode {
            id,
            element: Some(element),
            children: Vec::new(),
            parent: None,
            computed_layout: ComputedLayout::ZERO,
            query_key: None,
        }
    }
}
