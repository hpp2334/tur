use std::fmt;

use tur_shared::{Color, Constraints, Geometry, Offset, Size};

use crate::core::elements::ElementTree;
use crate::core::traits::ElementNodeId;

pub struct LayoutContext<'a> {
    pub(crate) tree: &'a mut ElementTree,
    node_id: ElementNodeId,
}

impl<'a> LayoutContext<'a> {
    pub(crate) fn new(tree: &'a mut ElementTree, node_id: ElementNodeId) -> Self {
        LayoutContext { tree, node_id }
    }

    pub fn layout_child(&mut self, child_id: ElementNodeId, constraints: &Constraints) -> Size {
        self.tree.layout_size(child_id, constraints)
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
}

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

pub trait Canvas: fmt::Debug {
    fn fill_geometry(&mut self, offset: Offset, geometry: &Geometry, color: &Color);
    fn fill_text(&mut self, offset: Offset, text: &str, font_size: f64, color: &Color);
}

pub trait Renderer {
    fn render(&mut self, tree: &ElementTree);

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn resize(&mut self, _logical_width: u32, _logical_height: u32, _dpr: f64) {}
}
