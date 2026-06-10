use parley::{FontContext, LayoutContext as ParleyLayoutContext};
use tur_shared::{Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::fonts::FontManager;
use crate::core::resource::{ResourceId, ResourceMap};

pub struct LayoutContext<'a> {
    pub(crate) tree: &'a mut ElementTree,
    node_id: ElementNodeId,
    font_manager: &'a mut FontManager,
    text_layout_cx: &'a mut ParleyLayoutContext<[u8; 4]>,
    resource_map: &'a ResourceMap,
}

impl<'a> LayoutContext<'a> {
    pub(crate) fn new(
        tree: &'a mut ElementTree,
        node_id: ElementNodeId,
        font_manager: &'a mut FontManager,
        text_layout_cx: &'a mut ParleyLayoutContext<[u8; 4]>,
        resource_map: &'a ResourceMap,
    ) -> Self {
        LayoutContext {
            tree,
            node_id,
            font_manager,
            text_layout_cx,
            resource_map,
        }
    }

    pub fn layout_child(&mut self, child_id: ElementNodeId, constraints: &Constraints) -> Size {
        self.tree.layout_size(
            child_id,
            constraints,
            self.font_manager,
            self.text_layout_cx,
            self.resource_map,
        )
    }

    pub fn set_child_offset(&mut self, child_id: ElementNodeId, offset: Offset) {
        if let Some(node) = self.tree.nodes.get_mut(&child_id) {
            node.computed_layout.offset = offset;
        }
    }

    pub fn set_child_offset_self(&mut self, offset: Offset) {
        if let Some(node) = self.tree.nodes.get_mut(&self.node_id) {
            node.computed_layout.offset = offset;
        }
    }

    pub fn child_type_name(&self, child_id: ElementNodeId) -> &'static str {
        self.tree
            .nodes
            .get(&child_id)
            .and_then(|n| n.element.as_ref())
            .map(|e| e.type_name())
            .unwrap_or("tur_container")
    }

    pub fn child_computed_size(&self, child_id: ElementNodeId) -> Size {
        self.tree
            .nodes
            .get(&child_id)
            .map(|n| n.computed_layout.size)
            .unwrap_or(Size::ZERO)
    }

    pub fn child_element<T: 'static>(&self, child_id: ElementNodeId) -> Option<&T> {
        self.tree
            .nodes
            .get(&child_id)
            .and_then(|n| n.element.as_ref())
            .and_then(|e| e.cast::<T>())
    }

    pub fn text_layout_contexts(
        &mut self,
    ) -> (&mut FontContext, &mut ParleyLayoutContext<[u8; 4]>) {
        (self.font_manager.font_context(), self.text_layout_cx)
    }

    pub fn get_image_natural_size(&self, resource_id: ResourceId) -> Option<Size> {
        self.resource_map.get_image(resource_id).map(|r| r.natural_size)
    }
}
