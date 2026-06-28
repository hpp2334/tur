use tur_shared::Offset;

use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;

pub struct HitTest<'a> {
    tree: &'a ElementTree,
}

impl<'a> HitTest<'a> {
    pub fn new(tree: &'a ElementTree) -> Self {
        Self { tree }
    }

    pub fn deepest(&self, position: Offset) -> Option<ElementNodeId> {
        self.tree.hit_test_path(position).first().copied()
    }

    pub fn path(&self, position: Offset) -> Vec<ElementNodeId> {
        self.tree.hit_test_path(position)
    }

    pub fn contains(&self, position: Offset, id: ElementNodeId) -> bool {
        self.tree.hit_test_path(position).contains(&id)
    }
}
