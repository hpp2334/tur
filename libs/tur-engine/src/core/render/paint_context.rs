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
    /// Wall-clock millis since epoch at the start of this frame. Used by
    /// time-based paint effects (e.g. caret blink phase). Sourced from
    /// `boa_engine::Context::clock().now().millis_since_epoch()` once per
    /// frame and threaded through every paint call.
    now_ms: u64,
}

impl<'a> PaintContext<'a> {
    pub(crate) fn new(
        tree: &'a ElementTree,
        focused_node_id: Option<ElementNodeId>,
        current_node_id: ElementNodeId,
        resource_map: &'a ResourceMap,
        now_ms: u64,
    ) -> Self {
        PaintContext {
            tree,
            resource_map,
            focused_node_id,
            current_node_id: Some(current_node_id),
            now_ms,
        }
    }

    pub fn paint_child(
        &self,
        child_id: ElementNodeId,
        canvas: &mut dyn Canvas,
        parent_offset: Offset,
    ) {
        self.tree.paint_node(child_id, canvas, parent_offset, self.focused_node_id, self.resource_map, self.now_ms);
    }

    pub fn is_focused(&self) -> bool {
        self.focused_node_id == self.current_node_id
    }

    /// Wall-clock millis since epoch sampled at the start of this frame.
    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    pub fn get_image_resource(&self, id: ResourceId) -> Option<&ImageResource> {
        self.resource_map.get_image(id)
    }
}
