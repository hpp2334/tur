use tur_shared::Offset;

use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::render::Canvas;
use crate::core::resource::{ImageResource, ResourceId, ResourceMap};

pub struct PaintContext<'a> {
    tree: &'a ElementTree,
    resource_map: &'a ResourceMap,
    focused_node_id: Option<ElementNodeId>,
    current_node_id: Option<ElementNodeId>,
}

impl<'a> PaintContext<'a> {
    pub(crate) fn new(
        tree: &'a ElementTree,
        focused_node_id: Option<ElementNodeId>,
        current_node_id: ElementNodeId,
        resource_map: &'a ResourceMap,
    ) -> Self {
        PaintContext {
            tree,
            resource_map,
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
        self.tree.paint_node(child_id, canvas, parent_offset, self.focused_node_id, self.resource_map);
    }

    pub fn is_focused(&self) -> bool {
        self.focused_node_id == self.current_node_id
    }

    pub fn get_image_resource(&self, id: ResourceId) -> Option<&ImageResource> {
        self.resource_map.get_image(id)
    }
}
