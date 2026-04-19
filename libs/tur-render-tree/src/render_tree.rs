use std::collections::HashMap;

use tur_element::{ElementNodeId, ElementTree};
use tur_shared::{ComputedLayout, Constraints, ElementKind, Offset, Size};

use crate::objects::create_render_object;
use crate::render_node::{RenderNode, RenderNodeId};
use crate::render_object::{ChildLayout, ChildPaint, PaintContext};

#[derive(Debug, Default)]
pub struct RenderTree {
    nodes: HashMap<RenderNodeId, RenderNode>,
    root_id: Option<RenderNodeId>,
}

struct TreeChildLayout<'a> {
    tree: &'a mut RenderTree,
    node_id: RenderNodeId,
}

impl ChildLayout for TreeChildLayout<'_> {
    fn layout_child(&mut self, child_id: RenderNodeId, constraints: &Constraints) -> Size {
        self.tree.layout_size(child_id, constraints)
    }

    fn set_child_offset(&mut self, child_id: RenderNodeId, offset: Offset) {
        if let Some(node) = self.tree.nodes.get_mut(&child_id) {
            node.computed_layout.offset = offset;
        }
    }

    fn set_child_offset_self(&mut self, offset: Offset) {
        if let Some(node) = self.tree.nodes.get_mut(&self.node_id) {
            node.computed_layout.offset = offset;
        }
    }

    fn get_child_kind(&self, child_id: RenderNodeId) -> ElementKind {
        self.tree
            .nodes
            .get(&child_id)
            .and_then(|n| n.object.as_ref())
            .map(|o| o.kind())
            .unwrap_or(ElementKind::Container)
    }
}

struct TreeChildPaint<'a> {
    tree: &'a RenderTree,
}

impl ChildPaint for TreeChildPaint<'_> {
    fn paint_child(
        &mut self,
        child_id: RenderNodeId,
        ctx: &mut dyn PaintContext,
        parent_offset: Offset,
    ) {
        self.tree.paint_node(child_id, ctx, parent_offset);
    }
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
        element_id: ElementNodeId,
        render_tree: &mut RenderTree,
    ) {
        let element_node = match element_tree.get(element_id) {
            Some(n) => n,
            None => return,
        };

        let object = create_render_object(element_node.kind, &element_node.props);

        let render_node = RenderNode {
            id: RenderNodeId::new(element_node.id.as_u64()),
            object: Some(object),
            children: element_node
                .children
                .iter()
                .map(|c| RenderNodeId::new(c.as_u64()))
                .collect(),
            computed_layout: ComputedLayout::ZERO,
        };

        let children = render_node.children.clone();
        render_tree.nodes.insert(render_node.id, render_node);

        for child_id in children {
            Self::convert_node(
                element_tree,
                ElementNodeId::new(child_id.as_u64()),
                render_tree,
            );
        }
    }

    pub fn compute_layout(&mut self, constraints: &Constraints) -> Size {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return constraints.constrain(Size::ZERO),
        };

        self.clear_layouts(root_id);

        let size = self.layout_size(root_id, constraints);

        self.layout_position(root_id);

        size
    }

    fn clear_layouts(&mut self, id: RenderNodeId) {
        let children: Vec<RenderNodeId> = self
            .nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        if let Some(node) = self.nodes.get_mut(&id) {
            node.computed_layout = ComputedLayout::ZERO;
        }
        for child_id in children {
            self.clear_layouts(child_id);
        }
    }

    fn layout_size(&mut self, id: RenderNodeId, constraints: &Constraints) -> Size {
        let children = self
            .nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();

        let mut object = self
            .nodes
            .get_mut(&id)
            .and_then(|n| n.object.take())
            .expect("object missing during layout_size");

        let mut child_layout = TreeChildLayout {
            tree: self,
            node_id: id,
        };
        let size = object.perform_layout_size(constraints, &children, &mut child_layout);

        let constrained = constraints.constrain(size);
        let node = child_layout.tree.nodes.get_mut(&id).unwrap();
        node.object = Some(object);
        node.computed_layout.size = constrained;
        constrained
    }

    fn layout_position(&mut self, id: RenderNodeId) {
        let children = self
            .nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();

        {
            let mut object = self
                .nodes
                .get_mut(&id)
                .and_then(|n| n.object.take())
                .expect("object missing during layout_position");

            let mut child_layout = TreeChildLayout {
                tree: self,
                node_id: id,
            };
            object.perform_layout_position(&children, &mut child_layout);

            child_layout.tree.nodes.get_mut(&id).unwrap().object = Some(object);
        }

        for child_id in children {
            self.layout_position(child_id);
        }
    }

    pub fn paint(&self, ctx: &mut dyn PaintContext) {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return,
        };
        self.paint_node(root_id, ctx, Offset::ZERO);
    }

    fn paint_node(&self, id: RenderNodeId, ctx: &mut dyn PaintContext, parent_offset: Offset) {
        let node = match self.nodes.get(&id) {
            Some(n) => n,
            None => return,
        };

        let object = match node.object.as_ref() {
            Some(o) => o,
            None => return,
        };

        let absolute_offset = parent_offset + node.computed_layout.offset;

        let mut child_paint = TreeChildPaint { tree: self };
        object.paint(
            ctx,
            absolute_offset,
            &node.computed_layout,
            &node.children,
            &mut child_paint,
        );
    }

    pub fn hit_test(&self, position: Offset) -> bool {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return false,
        };
        self.hit_test_node(root_id, position)
    }

    fn hit_test_node(&self, id: RenderNodeId, position: Offset) -> bool {
        let node = match self.nodes.get(&id) {
            Some(n) => n,
            None => return false,
        };

        let object = match node.object.as_ref() {
            Some(o) => o,
            None => return false,
        };

        let local_position = Offset::new(
            position.x - node.computed_layout.offset.x,
            position.y - node.computed_layout.offset.y,
        );

        if !object.hit_test(local_position, &node.computed_layout) {
            return false;
        }

        for &child_id in node.children.iter().rev() {
            if self.hit_test_node(child_id, local_position) {
                return true;
            }
        }

        true
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
