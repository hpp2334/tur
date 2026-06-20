use std::fmt;

use boa_engine::Context;
use tur_shared::{ComputedLayout, Constraints};

use crate::core::bridge::{BoaOpaque, TurNodeHandle};
use crate::core::element::ElementNodeId;
use crate::core::elements::AnyElement;

pub struct ElementObject {
    pub id: ElementNodeId,
    pub element: Option<AnyElement>,
    pub children: Vec<ElementNodeId>,
    pub parent: Option<ElementNodeId>,
    pub computed_layout: ComputedLayout,
    pub query_key: Option<Vec<String>>,
    pub handle: BoaOpaque<TurNodeHandle>,
    pub dirty_layout: bool,
    pub dirty_paint: bool,
    pub last_constraints: Option<Constraints>,
}

impl fmt::Debug for ElementObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElementObject")
            .field("id", &self.id)
            .field("kind", &self.element.as_ref().map(|e| e.kind()))
            .field("children", &self.children)
            .field("parent", &self.parent)
            .finish()
    }
}

impl ElementObject {
    pub fn new(id: ElementNodeId, element: AnyElement, context: &mut Context) -> Self {
        ElementObject {
            handle: BoaOpaque::new(TurNodeHandle { id }, context),
            id,
            element: Some(element),
            children: Vec::new(),
            parent: None,
            computed_layout: ComputedLayout::ZERO,
            query_key: None,
            dirty_layout: true,
            dirty_paint: true,
            last_constraints: None,
        }
    }
}
