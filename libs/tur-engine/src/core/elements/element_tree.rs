use std::collections::HashMap;

use parley::LayoutContext as ParleyLayoutContext;
use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::elements::ElementObject;
use crate::core::fonts::FontManager;
use crate::core::layout::LayoutContext;
use crate::core::render::{Canvas, PaintContext};
use crate::core::resource::ResourceMap;

#[derive(Debug)]
pub struct ElementTree {
    pub(crate) nodes: HashMap<ElementNodeId, ElementObject>,
    root_id: Option<ElementNodeId>,
    next_id: u64,
}

impl Default for ElementTree {
    fn default() -> Self {
        Self::new()
    }
}

impl ElementTree {
    pub fn new() -> Self {
        ElementTree {
            nodes: HashMap::new(),
            root_id: None,
            next_id: 1,
        }
    }

    pub fn alloc_id(&mut self) -> ElementNodeId {
        let id = self.next_id;
        self.next_id += 1;
        ElementNodeId::new(id)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn insert(&mut self, node: ElementObject) {
        if self.root_id.is_none() {
            self.root_id = Some(node.id);
        }
        self.nodes.insert(node.id, node);
    }

    pub fn get(&self, id: ElementNodeId) -> Option<&ElementObject> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: ElementNodeId) -> Option<&mut ElementObject> {
        self.nodes.get_mut(&id)
    }

    pub fn remove(&mut self, id: ElementNodeId) -> Option<ElementObject> {
        let node = self.nodes.remove(&id)?;
        if self.root_id == Some(id) {
            self.root_id = None;
        }
        Some(node)
    }

    pub fn root_id(&self) -> Option<ElementNodeId> {
        self.root_id
    }

    pub fn root(&self) -> Option<&ElementObject> {
        self.root_id.and_then(|id| self.nodes.get(&id))
    }

    pub fn root_mut(&mut self) -> Option<&mut ElementObject> {
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

    pub fn compute_layout(
        &mut self,
        constraints: &Constraints,
        font_manager: &mut FontManager,
        text_layout_cx: &mut ParleyLayoutContext<[u8; 4]>,
        resource_map: &ResourceMap,
    ) -> Size {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return constraints.constrain(Size::ZERO),
        };

        self.clear_layouts(root_id);

        let size = self.layout_size(root_id, constraints, font_manager, text_layout_cx, resource_map);

        self.layout_position(root_id, font_manager, text_layout_cx, resource_map);

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

    pub(crate) fn layout_size(
        &mut self,
        id: ElementNodeId,
        constraints: &Constraints,
        font_manager: &mut FontManager,
        text_layout_cx: &mut ParleyLayoutContext<[u8; 4]>,
        resource_map: &ResourceMap,
    ) -> Size {
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

        let mut cx = LayoutContext::new(self, id, font_manager, text_layout_cx, resource_map);
        let size = element.perform_layout_size(constraints, &children, &mut cx);

        let constrained = constraints.constrain(size);
        let node = cx.tree.nodes.get_mut(&id).unwrap();
        node.element = Some(element);
        node.computed_layout.size = constrained;
        constrained
    }

    fn layout_position(
        &mut self,
        id: ElementNodeId,
        font_manager: &mut FontManager,
        text_layout_cx: &mut ParleyLayoutContext<[u8; 4]>,
        resource_map: &ResourceMap,
    ) {
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

            let mut cx = LayoutContext::new(self, id, font_manager, text_layout_cx, resource_map);
            element.perform_layout_position(&children, &mut cx);

            cx.tree.nodes.get_mut(&id).unwrap().element = Some(element);
        }

        for child_id in children {
            self.layout_position(child_id, font_manager, text_layout_cx, resource_map);
        }
    }

    pub fn paint(&self, canvas: &mut dyn Canvas, focused_node_id: Option<ElementNodeId>, resource_map: &ResourceMap) {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return,
        };
        self.paint_node(root_id, canvas, Offset::ZERO, focused_node_id, resource_map);
    }

    pub(crate) fn paint_node(
        &self,
        id: ElementNodeId,
        canvas: &mut dyn Canvas,
        parent_offset: Offset,
        focused_node_id: Option<ElementNodeId>,
        resource_map: &ResourceMap,
    ) {
        let node = match self.nodes.get(&id) {
            Some(n) => n,
            None => return,
        };

        let element = match node.element.as_ref() {
            Some(e) => e,
            None => return,
        };

        let absolute_offset = parent_offset + node.computed_layout.offset;

        let paint_ctx = PaintContext::new(self, focused_node_id, id, resource_map);
        element.paint(
            canvas,
            absolute_offset,
            &node.computed_layout,
            &node.children,
            &paint_ctx,
        );
    }

    pub fn hit_test(&self, position: Offset) -> bool {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return false,
        };
        self.hit_test_node(root_id, position)
    }

    pub fn hit_test_path(&self, position: Offset) -> Vec<ElementNodeId> {
        let mut path = Vec::new();
        if let Some(root_id) = self.root_id {
            self.collect_hit_path(root_id, position, &mut path);
        }
        path
    }

    pub fn query_element(&self, key: &[&str]) -> Option<ElementNodeId> {
        let root_id = self.root_id?;
        let mut result = None;
        self.query_element_recursive(root_id, key, &mut result);
        result
    }

    fn query_element_recursive(
        &self,
        id: ElementNodeId,
        key: &[&str],
        result: &mut Option<ElementNodeId>,
    ) {
        if result.is_some() {
            return;
        }
        let node = match self.nodes.get(&id) {
            Some(n) => n,
            None => return,
        };
        if node
            .query_key
            .as_ref()
            .map(|k| k.iter().map(|s| s.as_str()).eq(key.iter().copied()))
            .unwrap_or(false)
            && node.element.is_some()
        {
            *result = Some(id);
            return;
        }
        for &child_id in &node.children {
            self.query_element_recursive(child_id, key, result);
            if result.is_some() {
                return;
            }
        }
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

    fn collect_hit_path(
        &self,
        id: ElementNodeId,
        position: Offset,
        path: &mut Vec<ElementNodeId>,
    ) -> bool {
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

        node.children
            .iter()
            .rev()
            .any(|&child_id| self.collect_hit_path(child_id, local_position, path));

        path.push(id);

        true
    }

    pub fn debug_layout(&self) -> String {
        let root_id = match self.root_id {
            Some(id) => id,
            None => return String::new(),
        };
        let mut buf = String::new();
        self.debug_node(root_id, &mut buf, "", Offset::ZERO);
        buf
    }

    fn debug_node(
        &self,
        id: ElementNodeId,
        buf: &mut String,
        prefix: &str,
        parent_offset: Offset,
    ) {
        let node = match self.nodes.get(&id) {
            Some(n) => n,
            None => return,
        };
        let element = match node.element.as_ref() {
            Some(e) => e,
            None => return,
        };

        let label = element.trace_label();
        let label_str = if label.is_empty() {
            String::new()
        } else {
            format!(" {label}")
        };
        let query_key_str = match &node.query_key {
            Some(keys) if !keys.is_empty() => format!(" [{}]", keys.join(", ")),
            _ => String::new(),
        };
        let abs = parent_offset + node.computed_layout.offset;
        buf.push_str(&format!(
            "{}{}{}{} abs({:.1},{:.1}) {:.1}x{:.1}\n",
            prefix,
            element.type_name(),
            label_str,
            query_key_str,
            abs.x,
            abs.y,
            node.computed_layout.size.width,
            node.computed_layout.size.height,
        ));

        let child_count = node.children.len();
        for (i, &child_id) in node.children.iter().enumerate() {
            let last = i == child_count - 1;
            let child_prefix = if last { "└── " } else { "├── " };
            let nested_prefix = if last { "    " } else { "│   " };
            self.debug_node(
                child_id,
                buf,
                &format!("{}{}", prefix.trim_end_matches(child_prefix), nested_prefix),
                abs,
            );
        }
    }
}
