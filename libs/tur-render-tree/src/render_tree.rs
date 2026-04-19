use std::collections::HashMap;

use tur_trait::{
    ChildLayout, ChildPaint, ComputedLayout, Constraints, ElementTreeProvider, Offset, RenderNode,
    RenderNodeId, Size,
};

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

    fn get_child_type_name(&self, child_id: RenderNodeId) -> &'static str {
        self.tree
            .nodes
            .get(&child_id)
            .and_then(|n| n.object.as_ref())
            .map(|o| o.type_name())
            .unwrap_or("tur_container")
    }
}

struct TreeChildPaint<'a> {
    tree: &'a RenderTree,
}

impl ChildPaint for TreeChildPaint<'_> {
    fn paint_child(
        &mut self,
        child_id: RenderNodeId,
        ctx: &mut dyn tur_trait::PaintContext,
        parent_offset: Offset,
    ) {
        self.tree.paint_node(child_id, ctx, parent_offset);
    }
}

impl RenderTree {
    pub fn from_element_tree_provider(provider: &dyn ElementTreeProvider) -> Self {
        let mut render_tree = RenderTree {
            nodes: HashMap::new(),
            root_id: provider.root_id().map(RenderNodeId::new),
        };

        if let Some(root_id) = provider.root_id() {
            Self::convert_node(provider, root_id, &mut render_tree);
        }

        render_tree
    }

    pub fn rebuild_from_element_tree_provider(&mut self, provider: &dyn ElementTreeProvider) {
        self.nodes.clear();
        self.root_id = provider.root_id().map(RenderNodeId::new);

        if let Some(root_id) = provider.root_id() {
            Self::convert_node(provider, root_id, self);
        }
    }

    fn convert_node(
        provider: &dyn ElementTreeProvider,
        element_id: u64,
        render_tree: &mut RenderTree,
    ) {
        let element = provider.element_for(element_id);
        let object = element.to_render_object_boxed();
        let children = provider.children_of(element_id);

        let render_node = RenderNode {
            id: RenderNodeId::new(element_id),
            object: Some(object),
            children: children.iter().map(|&c| RenderNodeId::new(c)).collect(),
            computed_layout: ComputedLayout::ZERO,
        };

        let child_ids = render_node.children.clone();
        render_tree.nodes.insert(render_node.id, render_node);

        for child_id in child_ids {
            Self::convert_node(provider, child_id.as_u64(), render_tree);
        }
    }

    pub fn insert(&mut self, node: RenderNode) {
        if self.root_id.is_none() {
            self.root_id = Some(node.id);
        }
        self.nodes.insert(node.id, node);
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

    pub fn paint(&self, ctx: &mut dyn tur_trait::PaintContext) {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return,
        };
        self.paint_node(root_id, ctx, Offset::ZERO);
    }

    fn paint_node(
        &self,
        id: RenderNodeId,
        ctx: &mut dyn tur_trait::PaintContext,
        parent_offset: Offset,
    ) {
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
