use std::collections::HashMap;

use tur_element::ElementTree;
use tur_shared::ComputedLayout;

use crate::render_node::{RenderNode, RenderNodeId};

#[derive(Debug, Default, Clone)]
pub struct RenderTree {
    nodes: HashMap<RenderNodeId, RenderNode>,
    root_id: Option<RenderNodeId>,
}

impl RenderTree {
    pub fn from_element_tree(element_tree: &ElementTree) -> Self {
        let mut render_tree = RenderTree {
            nodes: HashMap::new(),
            root_id: element_tree
                .root_id()
                .map(|id| RenderNodeId::new(id.as_u64())),
        };

        if let Some(root_id) = element_tree.root_id() {
            Self::convert_node(element_tree, root_id, &mut render_tree);
        }

        render_tree
    }

    pub fn rebuild_from_element_tree(&mut self, element_tree: &ElementTree) {
        self.nodes.clear();
        self.root_id = element_tree
            .root_id()
            .map(|id| RenderNodeId::new(id.as_u64()));

        if let Some(root_id) = element_tree.root_id() {
            Self::convert_node(element_tree, root_id, self);
        }
    }

    fn convert_node(
        element_tree: &ElementTree,
        element_id: tur_element::ElementNodeId,
        render_tree: &mut RenderTree,
    ) {
        let element_node = match element_tree.get(element_id) {
            Some(n) => n,
            None => return,
        };

        let computed_layout = element_node.computed_layout.unwrap_or(ComputedLayout::ZERO);

        let render_node = RenderNode {
            id: RenderNodeId::new(element_node.id.as_u64()),
            kind: element_node.kind,
            children: element_node
                .children
                .iter()
                .map(|c| RenderNodeId::new(c.as_u64()))
                .collect(),
            computed_layout,
            text_content: element_node.prop_str("content").map(String::from),
            font_size: element_node.prop_f64("fontSize"),
            color: element_node.prop_str("color").map(String::from),
            padding: element_node.prop_f64("padding"),
        };

        let children = render_node.children.clone();
        render_tree.nodes.insert(render_node.id, render_node);

        for child_id in children {
            Self::convert_node(
                element_tree,
                tur_element::ElementNodeId::new(child_id.as_u64()),
                render_tree,
            );
        }
    }

    pub fn get(&self, id: RenderNodeId) -> Option<&RenderNode> {
        self.nodes.get(&id)
    }

    pub fn root_id(&self) -> Option<RenderNodeId> {
        self.root_id
    }

    pub fn root(&self) -> Option<&RenderNode> {
        self.root_id.and_then(|id| self.nodes.get(&id))
    }
}
