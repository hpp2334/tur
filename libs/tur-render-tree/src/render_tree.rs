use std::collections::HashMap;

use tur_layout::{LayoutNodeId, LayoutTree};
use tur_shared::ComputedLayout;
use tur_widget::WidgetTree;

use crate::render_node::{RenderNode, RenderNodeId};

#[derive(Debug, Default, Clone)]
pub struct RenderTree {
    nodes: HashMap<RenderNodeId, RenderNode>,
    root_id: Option<RenderNodeId>,
}

impl RenderTree {
    pub fn from_layout_tree(layout_tree: &LayoutTree, widget_tree: &WidgetTree) -> Self {
        let mut render_tree = RenderTree {
            nodes: HashMap::new(),
            root_id: layout_tree
                .root_id()
                .map(|id| RenderNodeId::new(id.as_u64())),
        };

        if let Some(root_id) = layout_tree.root_id() {
            Self::convert_node(layout_tree, widget_tree, root_id, &mut render_tree);
        }

        render_tree
    }

    pub fn rebuild_from_layout_tree(&mut self, layout_tree: &LayoutTree, widget_tree: &WidgetTree) {
        self.nodes.clear();
        self.root_id = layout_tree
            .root_id()
            .map(|id| RenderNodeId::new(id.as_u64()));

        if let Some(root_id) = layout_tree.root_id() {
            Self::convert_node(layout_tree, widget_tree, root_id, self);
        }
    }

    fn convert_node(
        layout_tree: &LayoutTree,
        widget_tree: &WidgetTree,
        layout_id: LayoutNodeId,
        render_tree: &mut RenderTree,
    ) {
        let layout_node = match layout_tree.get_node(layout_id) {
            Some(n) => n,
            None => return,
        };

        let widget_id = tur_widget::WidgetNodeId::new(layout_node.id.as_u64());
        let widget_node = widget_tree.get(widget_id);

        let computed_layout = layout_node.computed_layout.unwrap_or(ComputedLayout::ZERO);

        let (text_content, font_size, color, padding) = match widget_node {
            Some(wn) => (
                wn.prop_str("content").map(String::from),
                wn.prop_f64("fontSize"),
                wn.prop_str("color").map(String::from),
                wn.prop_f64("padding"),
            ),
            None => (None, None, None, None),
        };

        let render_node = RenderNode {
            id: RenderNodeId::new(layout_node.id.as_u64()),
            kind: layout_node.kind,
            children: layout_node
                .children
                .iter()
                .map(|c| RenderNodeId::new(c.as_u64()))
                .collect(),
            computed_layout,
            text_content,
            font_size,
            color,
            padding,
        };

        let children = render_node.children.clone();
        render_tree.nodes.insert(render_node.id, render_node);

        for child_id in children {
            Self::convert_node(
                layout_tree,
                widget_tree,
                LayoutNodeId::new(child_id.as_u64()),
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
