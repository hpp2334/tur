use std::collections::HashMap;

use crate::layout::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WidgetKind {
    Column,
    Row,
    Expanded,
    Stack,
    Positioned,
    SizedBox,
    Container,
    Text,
}

impl WidgetKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Column" => Some(WidgetKind::Column),
            "Row" => Some(WidgetKind::Row),
            "Expanded" => Some(WidgetKind::Expanded),
            "Stack" => Some(WidgetKind::Stack),
            "Positioned" => Some(WidgetKind::Positioned),
            "SizedBox" => Some(WidgetKind::SizedBox),
            "Container" => Some(WidgetKind::Container),
            "Text" => Some(WidgetKind::Text),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PropValue {
    String(String),
    Number(f64),
    Bool(bool),
}

impl PropValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            PropValue::Number(n) => Some(*n),
            PropValue::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PropValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ComputedLayout {
    pub size: Size,
    pub offset: Offset,
}

#[derive(Debug)]
pub struct WidgetNode {
    pub id: u64,
    pub kind: WidgetKind,
    pub props: HashMap<String, PropValue>,
    pub children: Vec<u64>,
    pub parent: Option<u64>,
    pub computed_layout: Option<ComputedLayout>,
}

impl WidgetNode {
    pub fn new(id: u64, kind: WidgetKind) -> Self {
        WidgetNode {
            id,
            kind,
            props: HashMap::new(),
            children: Vec::new(),
            parent: None,
            computed_layout: None,
        }
    }

    pub fn set_prop(&mut self, key: String, value: PropValue) {
        self.props.insert(key, value);
    }

    pub fn get_prop(&self, key: &str) -> Option<&PropValue> {
        self.props.get(key)
    }

    pub fn prop_str(&self, key: &str) -> Option<&str> {
        self.props.get(key).and_then(|v| v.as_str())
    }

    pub fn prop_f64(&self, key: &str) -> Option<f64> {
        self.props.get(key).and_then(|v| v.as_f64())
    }
}

#[derive(Debug, Default)]
pub struct WidgetTree {
    nodes: HashMap<u64, WidgetNode>,
    root_id: Option<u64>,
}

impl WidgetTree {
    pub fn new() -> Self {
        WidgetTree {
            nodes: HashMap::new(),
            root_id: None,
        }
    }

    pub fn insert(&mut self, node: WidgetNode) {
        if self.root_id.is_none() {
            self.root_id = Some(node.id);
        }
        self.nodes.insert(node.id, node);
    }

    pub fn get(&self, id: u64) -> Option<&WidgetNode> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut WidgetNode> {
        self.nodes.get_mut(&id)
    }

    pub fn remove(&mut self, id: u64) -> Option<WidgetNode> {
        let node = self.nodes.remove(&id)?;
        if self.root_id == Some(id) {
            self.root_id = None;
        }
        Some(node)
    }

    pub fn root_id(&self) -> Option<u64> {
        self.root_id
    }

    pub fn root(&self) -> Option<&WidgetNode> {
        self.root_id.and_then(|id| self.nodes.get(&id))
    }

    pub fn root_mut(&mut self) -> Option<&mut WidgetNode> {
        self.root_id.and_then(|id| self.nodes.get_mut(&id))
    }

    pub fn set_root(&mut self, id: u64) {
        self.root_id = Some(id);
    }

    pub fn append_child(&mut self, parent_id: u64, child_id: u64) -> bool {
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

    pub fn remove_child(&mut self, parent_id: u64, child_id: u64) -> bool {
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

    pub fn insert_before(&mut self, parent_id: u64, child_id: u64, ref_id: u64) -> bool {
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

    pub fn children_of(&self, id: u64) -> Vec<u64> {
        self.nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default()
    }

    pub fn parent_of(&self, id: u64) -> Option<u64> {
        self.nodes.get(&id).and_then(|n| n.parent)
    }

    pub fn first_child_of(&self, id: u64) -> Option<u64> {
        self.nodes
            .get(&id)
            .and_then(|n| n.children.first().copied())
    }

    pub fn next_sibling_of(&self, id: u64) -> Option<u64> {
        let parent_id = self.nodes.get(&id).and_then(|n| n.parent)?;
        let parent = self.nodes.get(&parent_id)?;
        let pos = parent.children.iter().position(|&c| c == id)?;
        parent.children.get(pos + 1).copied()
    }
}

#[derive(Debug)]
pub struct LayoutResult {
    pub size: Size,
}

pub fn layout_tree(tree: &mut WidgetTree, constraints: &Constraints) -> LayoutResult {
    let root_id = match tree.root_id {
        Some(id) => id,
        None => {
            return LayoutResult {
                size: constraints.constrain(Size::ZERO),
            }
        }
    };
    let size = layout_node(tree, root_id, constraints);
    LayoutResult { size }
}

fn layout_node(tree: &mut WidgetTree, id: u64, constraints: &Constraints) -> Size {
    let kind = {
        let node = tree.get(id);
        match node {
            Some(n) => n.kind,
            None => return constraints.constrain(Size::ZERO),
        }
    };

    let size = match kind {
        WidgetKind::Column => layout_flex(tree, id, constraints, Axis::Vertical),
        WidgetKind::Row => layout_flex(tree, id, constraints, Axis::Horizontal),
        WidgetKind::Expanded => layout_expanded(tree, id, constraints),
        WidgetKind::Stack => layout_stack(tree, id, constraints),
        WidgetKind::Positioned => layout_positioned(tree, id, constraints),
        WidgetKind::SizedBox => layout_sized_box(tree, id, constraints),
        WidgetKind::Container => layout_container(tree, id, constraints),
        WidgetKind::Text => layout_text(tree, id, constraints),
    };

    let constrained = constraints.constrain(size);
    if let Some(node) = tree.get_mut(id) {
        node.computed_layout = Some(ComputedLayout {
            size: constrained,
            offset: Offset::ZERO,
        });
    }
    constrained
}

fn layout_flex(tree: &mut WidgetTree, id: u64, constraints: &Constraints, direction: Axis) -> Size {
    let child_ids = tree.children_of(id);

    let main_alignment = tree
        .get(id)
        .and_then(|n| n.prop_str("mainAlignment"))
        .and_then(|s| match s {
            "start" => Some(MainAxisAlignment::Start),
            "center" => Some(MainAxisAlignment::Center),
            "end" => Some(MainAxisAlignment::End),
            "space-between" => Some(MainAxisAlignment::SpaceBetween),
            "space-around" => Some(MainAxisAlignment::SpaceAround),
            "space-evenly" => Some(MainAxisAlignment::SpaceEvenly),
            _ => None,
        })
        .unwrap_or(MainAxisAlignment::Start);

    let cross_alignment = tree
        .get(id)
        .and_then(|n| n.prop_str("crossAlignment"))
        .and_then(|s| match s {
            "start" => Some(CrossAxisAlignment::Start),
            "center" => Some(CrossAxisAlignment::Center),
            "end" => Some(CrossAxisAlignment::End),
            "stretch" => Some(CrossAxisAlignment::Stretch),
            _ => None,
        })
        .unwrap_or(CrossAxisAlignment::Center);

    let (mut total_main, mut max_cross): (f64, f64) = (0.0, 0.0);
    let mut flex_count = 0u32;
    let mut child_sizes: Vec<(u64, Size, bool)> = Vec::with_capacity(child_ids.len());

    for &child_id in &child_ids {
        let is_flex = tree
            .get(child_id)
            .map(|n| n.kind == WidgetKind::Expanded)
            .unwrap_or(false);

        if is_flex {
            flex_count += 1;
            child_sizes.push((child_id, Size::ZERO, true));
        } else {
            let child_constraints = match direction {
                Axis::Vertical => Constraints {
                    min_width: if cross_alignment == CrossAxisAlignment::Stretch {
                        constraints.max_width
                    } else {
                        0.0
                    },
                    max_width: constraints.max_width,
                    min_height: 0.0,
                    max_height: constraints.max_height - total_main,
                },
                Axis::Horizontal => Constraints {
                    min_width: 0.0,
                    max_width: constraints.max_width - total_main,
                    min_height: if cross_alignment == CrossAxisAlignment::Stretch {
                        constraints.max_height
                    } else {
                        0.0
                    },
                    max_height: constraints.max_height,
                },
            };
            let size = layout_node(tree, child_id, &child_constraints);
            total_main += direction.main(size);
            max_cross = max_cross.max(direction.cross(size));
            child_sizes.push((child_id, size, false));
        }
    }

    let available_main = direction.main(Constraints::constrain(
        constraints,
        Size::new(constraints.max_width, constraints.max_height),
    ));
    let remaining_main = (available_main - total_main).max(0.0);
    let flex_space = if flex_count > 0 {
        remaining_main / flex_count as f64
    } else {
        0.0
    };

    for entry in &mut child_sizes {
        if entry.2 {
            let child_id = entry.0;
            let child_constraints = match direction {
                Axis::Vertical => Constraints {
                    min_width: if cross_alignment == CrossAxisAlignment::Stretch {
                        constraints.max_width
                    } else {
                        0.0
                    },
                    max_width: constraints.max_width,
                    min_height: flex_space,
                    max_height: flex_space,
                },
                Axis::Horizontal => Constraints {
                    min_width: flex_space,
                    max_width: flex_space,
                    min_height: if cross_alignment == CrossAxisAlignment::Stretch {
                        constraints.max_height
                    } else {
                        0.0
                    },
                    max_height: constraints.max_height,
                },
            };
            let size = layout_node(tree, child_id, &child_constraints);
            entry.1 = size;
            max_cross = max_cross.max(direction.cross(size));
        }
    }

    let total_main: f64 = child_sizes.iter().map(|(_, s, _)| direction.main(*s)).sum();
    let allocated_main = total_main;
    let mut current_main = match main_alignment {
        MainAxisAlignment::Start | MainAxisAlignment::SpaceBetween => 0.0,
        MainAxisAlignment::Center => (available_main - allocated_main) / 2.0,
        MainAxisAlignment::End => available_main - allocated_main,
        MainAxisAlignment::SpaceAround => {
            if child_sizes.is_empty() {
                0.0
            } else {
                (available_main - allocated_main) / (child_sizes.len() as f64 * 2.0)
            }
        }
        MainAxisAlignment::SpaceEvenly => {
            if child_sizes.is_empty() {
                0.0
            } else {
                (available_main - allocated_main) / ((child_sizes.len() + 1) as f64)
            }
        }
    };

    let gap = match main_alignment {
        MainAxisAlignment::SpaceBetween if child_sizes.len() > 1 => {
            (available_main - allocated_main) / ((child_sizes.len() - 1) as f64)
        }
        MainAxisAlignment::SpaceAround => {
            (available_main - allocated_main) / (child_sizes.len() as f64 * 2.0) * 2.0
        }
        MainAxisAlignment::SpaceEvenly => {
            (available_main - allocated_main) / ((child_sizes.len() + 1) as f64)
        }
        _ => 0.0,
    };

    for (i, (child_id, child_size, _)) in child_sizes.iter().enumerate() {
        let cross = match cross_alignment {
            CrossAxisAlignment::Start => 0.0,
            CrossAxisAlignment::Center => {
                (direction.cross(Constraints::constrain(
                    constraints,
                    Size::new(constraints.max_width, constraints.max_height),
                )) - direction.cross(*child_size))
                    / 2.0
            }
            CrossAxisAlignment::End => {
                direction.cross(Constraints::constrain(
                    constraints,
                    Size::new(constraints.max_width, constraints.max_height),
                )) - direction.cross(*child_size)
            }
            CrossAxisAlignment::Stretch => 0.0,
        };

        let offset = match direction {
            Axis::Vertical => Offset::new(cross, current_main),
            Axis::Horizontal => Offset::new(current_main, cross),
        };

        if let Some(child) = tree.get_mut(*child_id) {
            if let Some(ref mut cl) = child.computed_layout {
                cl.offset = offset;
            } else {
                child.computed_layout = Some(ComputedLayout {
                    size: *child_size,
                    offset,
                });
            }
        }

        current_main += direction.main(*child_size);
        if i < child_sizes.len() - 1 {
            current_main += gap;
        }
    }

    let size = match direction {
        Axis::Vertical => Size::new(
            max_cross.clamp(constraints.min_width, constraints.max_width),
            total_main.clamp(constraints.min_height, constraints.max_height),
        ),
        Axis::Horizontal => Size::new(
            total_main.clamp(constraints.min_width, constraints.max_width),
            max_cross.clamp(constraints.min_height, constraints.max_height),
        ),
    };

    constraints.constrain(size)
}

fn layout_expanded(tree: &mut WidgetTree, id: u64, constraints: &Constraints) -> Size {
    let child_ids = tree.children_of(id);
    if let Some(&child_id) = child_ids.first() {
        layout_node(tree, child_id, constraints)
    } else {
        constraints.constrain(Size::ZERO)
    }
}

fn layout_stack(tree: &mut WidgetTree, id: u64, constraints: &Constraints) -> Size {
    let child_ids = tree.children_of(id);
    let stack_fit = tree
        .get(id)
        .and_then(|n| n.prop_str("fit"))
        .and_then(|s| match s {
            "loose" => Some(StackFit::Loose),
            "expand" => Some(StackFit::Expand),
            "passthrough" => Some(StackFit::Passthrough),
            _ => None,
        })
        .unwrap_or(StackFit::Loose);

    let mut max_size = Size::ZERO;

    for &child_id in &child_ids {
        let child_constraints = match stack_fit {
            StackFit::Loose => Constraints::loose(
                constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
            ),
            StackFit::Expand => Constraints::tight(
                constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
            ),
            StackFit::Passthrough => *constraints,
        };
        let size = layout_node(tree, child_id, &child_constraints);
        max_size = Size::new(
            max_size.width.max(size.width),
            max_size.height.max(size.height),
        );
    }

    let stack_size = constraints.constrain(max_size);

    for &child_id in &child_ids {
        let is_positioned = tree
            .get(child_id)
            .map(|n| n.kind == WidgetKind::Positioned)
            .unwrap_or(false);

        if !is_positioned {
            if let Some(child) = tree.get_mut(child_id) {
                if let Some(ref mut cl) = child.computed_layout {
                    cl.offset = Offset::ZERO;
                }
            }
        }
    }

    stack_size
}

fn layout_positioned(tree: &mut WidgetTree, id: u64, constraints: &Constraints) -> Size {
    let child_ids = tree.children_of(id);

    let left = tree.get(id).and_then(|n| n.prop_f64("left"));
    let top = tree.get(id).and_then(|n| n.prop_f64("top"));
    let right = tree.get(id).and_then(|n| n.prop_f64("right"));
    let bottom = tree.get(id).and_then(|n| n.prop_f64("bottom"));

    let child_constraints = match (left, right, top, bottom) {
        (Some(_), Some(_), Some(_), Some(_)) => {
            let w = (constraints.max_width - left.unwrap_or(0.0) - right.unwrap_or(0.0)).max(0.0);
            let h = (constraints.max_height - top.unwrap_or(0.0) - bottom.unwrap_or(0.0)).max(0.0);
            Constraints::tight(Size::new(w, h))
        }
        _ => Constraints::loose(
            constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
        ),
    };

    let child_size = if let Some(&child_id) = child_ids.first() {
        layout_node(tree, child_id, &child_constraints)
    } else {
        child_constraints.constrain(Size::ZERO)
    };

    let offset_x = left.unwrap_or(0.0);
    let offset_y = top.unwrap_or(0.0);

    if let Some(node) = tree.get_mut(id) {
        node.computed_layout = Some(ComputedLayout {
            size: child_size,
            offset: Offset::new(offset_x, offset_y),
        });
    }

    child_size
}

fn layout_sized_box(tree: &mut WidgetTree, id: u64, constraints: &Constraints) -> Size {
    let width = tree.get(id).and_then(|n| n.prop_f64("width"));
    let height = tree.get(id).and_then(|n| n.prop_f64("height"));

    let child_ids = tree.children_of(id);
    let child_constraints = Constraints {
        min_width: width.unwrap_or(constraints.min_width),
        max_width: width.unwrap_or(constraints.max_width),
        min_height: height.unwrap_or(constraints.min_height),
        max_height: height.unwrap_or(constraints.max_height),
    };

    if let Some(&child_id) = child_ids.first() {
        layout_node(tree, child_id, &child_constraints)
    } else {
        child_constraints.constrain(Size::ZERO)
    }
}

fn layout_container(tree: &mut WidgetTree, id: u64, constraints: &Constraints) -> Size {
    let padding = tree.get(id).and_then(|n| {
        let v = n.prop_f64("padding")?;
        Some(EdgeInsets::all(v))
    });

    let child_ids = tree.children_of(id);
    let inner_constraints = match padding {
        Some(p) => constraints.deflate(p),
        None => *constraints,
    };

    let child_size = if let Some(&child_id) = child_ids.first() {
        layout_node(tree, child_id, &inner_constraints)
    } else {
        inner_constraints.constrain(Size::ZERO)
    };

    match padding {
        Some(p) => p.inflate_size(child_size),
        None => child_size,
    }
}

fn layout_text(tree: &mut WidgetTree, id: u64, constraints: &Constraints) -> Size {
    let content = tree
        .get(id)
        .and_then(|n| n.prop_str("content"))
        .unwrap_or("");
    let font_size = tree
        .get(id)
        .and_then(|n| n.prop_f64("fontSize"))
        .unwrap_or(14.0);

    let char_width = font_size * 0.6;
    let line_height = font_size * 1.2;

    let max_width = constraints.max_width;
    let chars_per_line = if max_width.is_finite() && max_width > 0.0 {
        (max_width / char_width).max(1.0) as usize
    } else {
        content.len().max(1)
    };

    let lines = if chars_per_line > 0 && !content.is_empty() {
        (content.len() as f64 / chars_per_line as f64).ceil() as usize
    } else {
        1
    };

    let width = if content.is_empty() {
        0.0
    } else if max_width.is_finite() {
        let actual_chars = content.len().min(chars_per_line);
        actual_chars as f64 * char_width
    } else {
        content.len() as f64 * char_width
    };

    let height = lines as f64 * line_height;

    constraints.constrain(Size::new(width, height))
}
