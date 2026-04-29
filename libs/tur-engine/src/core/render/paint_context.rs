use tur_shared::Offset;

use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::render::Canvas;

pub struct PaintContext<'a> {
    tree: &'a ElementTree,
    focused_node_id: Option<ElementNodeId>,
    current_node_id: Option<ElementNodeId>,
}

impl<'a> PaintContext<'a> {
    pub(crate) fn new(
        tree: &'a ElementTree,
        focused_node_id: Option<ElementNodeId>,
        current_node_id: ElementNodeId,
    ) -> Self {
        PaintContext {
            tree,
            focused_node_id,
            current_node_id: Some(current_node_id),
        }
    }

    pub fn paint_child(
        &self,
        child_id: ElementNodeId,
        canvas: &mut dyn Canvas,
        parent_offset: Offset,
    ) {
        self.tree.paint_node(child_id, canvas, parent_offset, self.focused_node_id);
    }

    pub fn is_focused(&self) -> bool {
        self.focused_node_id == self.current_node_id
    }
}
