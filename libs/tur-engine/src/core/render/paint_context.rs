use tur_shared::Offset;

use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::render::Canvas;

pub struct PaintContext<'a> {
    tree: &'a ElementTree,
}

impl<'a> PaintContext<'a> {
    pub(crate) fn new(tree: &'a ElementTree) -> Self {
        PaintContext { tree }
    }

    pub fn paint_child(
        &self,
        child_id: ElementNodeId,
        canvas: &mut dyn Canvas,
        parent_offset: Offset,
    ) {
        self.tree.paint_node(child_id, canvas, parent_offset);
    }
}
