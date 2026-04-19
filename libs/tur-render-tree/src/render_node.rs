use tur_shared::ComputedLayout;

use crate::render_object::RenderObject;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderNodeId(u64);

impl RenderNodeId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

pub struct RenderNode {
    pub id: RenderNodeId,
    pub object: Option<Box<dyn RenderObject>>,
    pub children: Vec<RenderNodeId>,
    pub computed_layout: ComputedLayout,
}

impl RenderNode {
    pub fn new(id: RenderNodeId, object: Box<dyn RenderObject>) -> Self {
        RenderNode {
            id,
            object: Some(object),
            children: Vec::new(),
            computed_layout: ComputedLayout::ZERO,
        }
    }
}

impl std::fmt::Debug for RenderNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderNode")
            .field("id", &self.id)
            .field("object", &self.object)
            .field("children", &self.children)
            .field("computed_layout", &self.computed_layout)
            .finish()
    }
}
