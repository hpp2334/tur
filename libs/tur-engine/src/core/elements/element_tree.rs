use std::collections::HashMap;

use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::elements::ElementNode;
use crate::core::render::{ChildLayout, ChildPaint, PaintContext};
use crate::core::traits::ElementNodeId;

#[derive(Debug, Default)]
pub struct ElementTree {
    nodes: HashMap<ElementNodeId, ElementNode>,
    root_id: Option<ElementNodeId>,
}

struct TreeChildLayout<'a> {
    tree: &'a mut ElementTree,
    node_id: ElementNodeId,
}

impl ChildLayout for TreeChildLayout<'_> {
    fn layout_child(&mut self, child_id: ElementNodeId, constraints: &Constraints) -> Size {
        self.tree.layout_size(child_id, constraints)
    }

    fn set_child_offset(&mut self, child_id: ElementNodeId, offset: Offset) {
        if let Some(node) = self.tree.nodes.get_mut(&child_id) {
            node.computed_layout.offset = offset;
        }
    }

    fn set_child_offset_self(&mut self, offset: Offset) {
        if let Some(node) = self.tree.nodes.get_mut(&self.node_id) {
            node.computed_layout.offset = offset;
        }
    }

    fn get_child_type_name(&self, child_id: ElementNodeId) -> &'static str {
        self.tree
            .nodes
            .get(&child_id)
            .and_then(|n| n.element.as_ref())
            .map(|e| e.type_name())
            .unwrap_or("tur_container")
    }
}

struct TreeChildPaint<'a> {
    tree: &'a ElementTree,
}

impl ChildPaint for TreeChildPaint<'_> {
    fn paint_child(
        &mut self,
        child_id: ElementNodeId,
        ctx: &mut dyn PaintContext,
        parent_offset: Offset,
    ) {
        self.tree.paint_node(child_id, ctx, parent_offset);
    }
}

impl ElementTree {
    pub fn new() -> Self {
        ElementTree {
            nodes: HashMap::new(),
            root_id: None,
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn insert(&mut self, node: ElementNode) {
        if self.root_id.is_none() {
            self.root_id = Some(node.id);
        }
        self.nodes.insert(node.id, node);
    }

    pub fn get(&self, id: ElementNodeId) -> Option<&ElementNode> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: ElementNodeId) -> Option<&mut ElementNode> {
        self.nodes.get_mut(&id)
    }

    pub fn remove(&mut self, id: ElementNodeId) -> Option<ElementNode> {
        let node = self.nodes.remove(&id)?;
        if self.root_id == Some(id) {
            self.root_id = None;
        }
        Some(node)
    }

    pub fn root_id(&self) -> Option<ElementNodeId> {
        self.root_id
    }

    pub fn root(&self) -> Option<&ElementNode> {
        self.root_id.and_then(|id| self.nodes.get(&id))
    }

    pub fn root_mut(&mut self) -> Option<&mut ElementNode> {
        self.root_id.and_then(|id| self.nodes.get_mut(&id))
    }

    pub fn set_root(&mut self, id: ElementNodeId) {
        self.root_id = Some(id);
    }

    pub fn append_child(&mut self, parent_id: ElementNodeId, child_id: ElementNodeId) -> bool {
        if !self.nodes.contains_key(&parent_id) || !self.nodes.contains_key(&child_id) {
            return false;
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = Some(parent_id);
        }
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.push(child_id);
        }
        true
    }

    pub fn remove_child(&mut self, parent_id: ElementNodeId, child_id: ElementNodeId) -> bool {
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            if let Some(pos) = parent.children.iter().position(|&id| id == child_id) {
                parent.children.remove(pos);
            }
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = None;
        }
        true
    }

    pub fn insert_before(
        &mut self,
        parent_id: ElementNodeId,
        child_id: ElementNodeId,
        ref_id: ElementNodeId,
    ) -> bool {
        if !self.nodes.contains_key(&parent_id)
            || !self.nodes.contains_key(&child_id)
            || !self.nodes.contains_key(&ref_id)
        {
            return false;
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = Some(parent_id);
        }
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            if let Some(pos) = parent.children.iter().position(|&id| id == ref_id) {
                parent.children.insert(pos, child_id);
            } else {
                parent.children.push(child_id);
            }
        }
        true
    }

    pub fn children_of(&self, id: ElementNodeId) -> Vec<ElementNodeId> {
        self.nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default()
    }

    pub fn parent_of(&self, id: ElementNodeId) -> Option<ElementNodeId> {
        self.nodes.get(&id).and_then(|n| n.parent)
    }

    pub fn first_child_of(&self, id: ElementNodeId) -> Option<ElementNodeId> {
        self.nodes
            .get(&id)
            .and_then(|n| n.children.first().copied())
    }

    pub fn next_sibling_of(&self, id: ElementNodeId) -> Option<ElementNodeId> {
        let parent_id = self.nodes.get(&id).and_then(|n| n.parent)?;
        let parent = self.nodes.get(&parent_id)?;
        let pos = parent.children.iter().position(|&c| c == id)?;
        parent.children.get(pos + 1).copied()
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

    fn clear_layouts(&mut self, id: ElementNodeId) {
        let children: Vec<ElementNodeId> = self
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

    fn layout_size(&mut self, id: ElementNodeId, constraints: &Constraints) -> Size {
        let children = self
            .nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();

        let mut element = self
            .nodes
            .get_mut(&id)
            .and_then(|n| n.element.take())
            .expect("element missing during layout_size");

        let mut child_layout = TreeChildLayout {
            tree: self,
            node_id: id,
        };
        let size = element.perform_layout_size(constraints, &children, &mut child_layout);

        let constrained = constraints.constrain(size);
        let node = child_layout.tree.nodes.get_mut(&id).unwrap();
        node.element = Some(element);
        node.computed_layout.size = constrained;
        constrained
    }

    fn layout_position(&mut self, id: ElementNodeId) {
        let children = self
            .nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();

        {
            let mut element = self
                .nodes
                .get_mut(&id)
                .and_then(|n| n.element.take())
                .expect("element missing during layout_position");

            let mut child_layout = TreeChildLayout {
                tree: self,
                node_id: id,
            };
            element.perform_layout_position(&children, &mut child_layout);

            child_layout.tree.nodes.get_mut(&id).unwrap().element = Some(element);
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

    fn paint_node(&self, id: ElementNodeId, ctx: &mut dyn PaintContext, parent_offset: Offset) {
        let node = match self.nodes.get(&id) {
            Some(n) => n,
            None => return,
        };

        let element = match node.element.as_ref() {
            Some(e) => e,
            None => return,
        };

        let absolute_offset = parent_offset + node.computed_layout.offset;

        let mut child_paint = TreeChildPaint { tree: self };
        element.paint(
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

    fn hit_test_node(&self, id: ElementNodeId, position: Offset) -> bool {
        let node = match self.nodes.get(&id) {
            Some(n) => n,
            None => return false,
        };

        let element = match node.element.as_ref() {
            Some(e) => e,
            None => return false,
        };

        let local_position = Offset::new(
            position.x - node.computed_layout.offset.x,
            position.y - node.computed_layout.offset.y,
        );

        if !element.hit_test(local_position, &node.computed_layout) {
            return false;
        }

        for &child_id in node.children.iter().rev() {
            if self.hit_test_node(child_id, local_position) {
                return true;
            }
        }

        true
    }
}
