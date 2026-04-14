use std::collections::HashMap;

use tur_shared::{
    Axis, ComputedLayout, Constraints, CrossAxisAlignment, EdgeInsets, MainAxisAlignment, Offset,
    Size, StackFit,
};
use tur_widget::WidgetKind;

use crate::layout::LayoutResult;
use crate::LayoutNodeId;

#[derive(Debug)]
pub struct LayoutNode {
    pub id: LayoutNodeId,
    pub kind: WidgetKind,
    pub children: Vec<LayoutNodeId>,
    pub parent: Option<LayoutNodeId>,
    pub computed_layout: Option<ComputedLayout>,
    pub main_alignment: MainAxisAlignment,
    pub cross_alignment: CrossAxisAlignment,
    pub stack_fit: StackFit,
    pub left: Option<f64>,
    pub top: Option<f64>,
    pub right: Option<f64>,
    pub bottom: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub padding: Option<EdgeInsets>,
    pub text_content: Option<String>,
    pub font_size: Option<f64>,
}

impl LayoutNode {
    pub fn from_widget(node: &tur_widget::WidgetNode) -> Self {
        let main_alignment = node
            .prop_str("mainAlignment")
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

        let cross_alignment = node
            .prop_str("crossAlignment")
            .and_then(|s| match s {
                "start" => Some(CrossAxisAlignment::Start),
                "center" => Some(CrossAxisAlignment::Center),
                "end" => Some(CrossAxisAlignment::End),
                "stretch" => Some(CrossAxisAlignment::Stretch),
                _ => None,
            })
            .unwrap_or(CrossAxisAlignment::Center);

        let stack_fit = node
            .prop_str("fit")
            .and_then(|s| match s {
                "loose" => Some(StackFit::Loose),
                "expand" => Some(StackFit::Expand),
                "passthrough" => Some(StackFit::Passthrough),
                _ => None,
            })
            .unwrap_or(StackFit::Loose);

        let padding = node.prop_f64("padding").map(EdgeInsets::all);

        LayoutNode {
            id: LayoutNodeId::new(node.id.as_u64()),
            kind: node.kind,
            children: node
                .children
                .iter()
                .map(|c| LayoutNodeId::new(c.as_u64()))
                .collect(),
            parent: node.parent.map(|p| LayoutNodeId::new(p.as_u64())),
            computed_layout: None,
            main_alignment,
            cross_alignment,
            stack_fit,
            left: node.prop_f64("left"),
            top: node.prop_f64("top"),
            right: node.prop_f64("right"),
            bottom: node.prop_f64("bottom"),
            width: node.prop_f64("width"),
            height: node.prop_f64("height"),
            padding,
            text_content: node.prop_str("content").map(String::from),
            font_size: node.prop_f64("fontSize"),
        }
    }
}

#[derive(Debug, Default)]
pub struct LayoutTree {
    nodes: HashMap<LayoutNodeId, LayoutNode>,
    root_id: Option<LayoutNodeId>,
}

impl LayoutTree {
    pub fn from_widget_tree(tree: &tur_widget::WidgetTree) -> Self {
        let mut layout_tree = LayoutTree {
            nodes: HashMap::new(),
            root_id: tree.root_id().map(|id| LayoutNodeId::new(id.as_u64())),
        };

        if let Some(root_id) = tree.root_id() {
            Self::convert_node(tree, root_id, &mut layout_tree);
        }

        layout_tree
    }

    fn convert_node(
        tree: &tur_widget::WidgetTree,
        id: tur_widget::WidgetNodeId,
        layout_tree: &mut LayoutTree,
    ) {
        if let Some(node) = tree.get(id) {
            let layout_node = LayoutNode::from_widget(node);
            layout_tree.nodes.insert(layout_node.id, layout_node);
            for &child_id in &node.children {
                Self::convert_node(tree, child_id, layout_tree);
            }
        }
    }

    pub fn compute_layout(&mut self, constraints: &Constraints) -> LayoutResult {
        let root_id = match self.root_id {
            Some(id) => id,
            None => {
                return LayoutResult {
                    size: constraints.constrain(Size::ZERO),
                }
            }
        };
        let size = self.layout_node(root_id, constraints);
        LayoutResult { size }
    }

    pub fn get_node(&self, id: LayoutNodeId) -> Option<&LayoutNode> {
        self.nodes.get(&id)
    }

    pub fn root_id(&self) -> Option<LayoutNodeId> {
        self.root_id
    }

    fn get(&self, id: LayoutNodeId) -> Option<&LayoutNode> {
        self.nodes.get(&id)
    }

    fn get_mut(&mut self, id: LayoutNodeId) -> Option<&mut LayoutNode> {
        self.nodes.get_mut(&id)
    }

    fn children_of(&self, id: LayoutNodeId) -> Vec<LayoutNodeId> {
        self.nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default()
    }

    fn layout_node(&mut self, id: LayoutNodeId, constraints: &Constraints) -> Size {
        let kind = {
            let node = self.get(id);
            match node {
                Some(n) => n.kind,
                None => return constraints.constrain(Size::ZERO),
            }
        };

        let size = match kind {
            WidgetKind::Column => self.layout_flex(id, constraints, Axis::Vertical),
            WidgetKind::Row => self.layout_flex(id, constraints, Axis::Horizontal),
            WidgetKind::Expanded => self.layout_expanded(id, constraints),
            WidgetKind::Stack => self.layout_stack(id, constraints),
            WidgetKind::Positioned => self.layout_positioned(id, constraints),
            WidgetKind::SizedBox => self.layout_sized_box(id, constraints),
            WidgetKind::Container => self.layout_container(id, constraints),
            WidgetKind::Text => self.layout_text(id, constraints),
        };

        let constrained = constraints.constrain(size);
        if let Some(node) = self.get_mut(id) {
            node.computed_layout = Some(ComputedLayout {
                size: constrained,
                offset: Offset::ZERO,
            });
        }
        constrained
    }

    fn layout_flex(
        &mut self,
        id: LayoutNodeId,
        constraints: &Constraints,
        direction: Axis,
    ) -> Size {
        let child_ids = self.children_of(id);

        let main_alignment = self
            .get(id)
            .map(|n| n.main_alignment)
            .unwrap_or(MainAxisAlignment::Start);

        let cross_alignment = self
            .get(id)
            .map(|n| n.cross_alignment)
            .unwrap_or(CrossAxisAlignment::Center);

        let (mut total_main, mut max_cross): (f64, f64) = (0.0, 0.0);
        let mut flex_count = 0u32;
        let mut child_sizes: Vec<(LayoutNodeId, Size, bool)> = Vec::with_capacity(child_ids.len());

        for &child_id in &child_ids {
            let is_flex = self
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
                let size = self.layout_node(child_id, &child_constraints);
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
                let size = self.layout_node(child_id, &child_constraints);
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

            if let Some(child) = self.get_mut(*child_id) {
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

    fn layout_expanded(&mut self, id: LayoutNodeId, constraints: &Constraints) -> Size {
        let child_ids = self.children_of(id);
        if let Some(&child_id) = child_ids.first() {
            self.layout_node(child_id, constraints)
        } else {
            constraints.constrain(Size::ZERO)
        }
    }

    fn layout_stack(&mut self, id: LayoutNodeId, constraints: &Constraints) -> Size {
        let child_ids = self.children_of(id);
        let stack_fit = self.get(id).map(|n| n.stack_fit).unwrap_or(StackFit::Loose);

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
            let size = self.layout_node(child_id, &child_constraints);
            max_size = Size::new(
                max_size.width.max(size.width),
                max_size.height.max(size.height),
            );
        }

        let stack_size = constraints.constrain(max_size);

        for &child_id in &child_ids {
            let is_positioned = self
                .get(child_id)
                .map(|n| n.kind == WidgetKind::Positioned)
                .unwrap_or(false);

            if !is_positioned {
                if let Some(child) = self.get_mut(child_id) {
                    if let Some(ref mut cl) = child.computed_layout {
                        cl.offset = Offset::ZERO;
                    }
                }
            }
        }

        stack_size
    }

    fn layout_positioned(&mut self, id: LayoutNodeId, constraints: &Constraints) -> Size {
        let child_ids = self.children_of(id);

        let left = self.get(id).and_then(|n| n.left);
        let top = self.get(id).and_then(|n| n.top);
        let right = self.get(id).and_then(|n| n.right);
        let bottom = self.get(id).and_then(|n| n.bottom);

        let child_constraints = match (left, right, top, bottom) {
            (Some(_), Some(_), Some(_), Some(_)) => {
                let w =
                    (constraints.max_width - left.unwrap_or(0.0) - right.unwrap_or(0.0)).max(0.0);
                let h =
                    (constraints.max_height - top.unwrap_or(0.0) - bottom.unwrap_or(0.0)).max(0.0);
                Constraints::tight(Size::new(w, h))
            }
            _ => Constraints::loose(
                constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
            ),
        };

        let child_size = if let Some(&child_id) = child_ids.first() {
            self.layout_node(child_id, &child_constraints)
        } else {
            child_constraints.constrain(Size::ZERO)
        };

        let offset_x = left.unwrap_or(0.0);
        let offset_y = top.unwrap_or(0.0);

        if let Some(node) = self.get_mut(id) {
            node.computed_layout = Some(ComputedLayout {
                size: child_size,
                offset: Offset::new(offset_x, offset_y),
            });
        }

        child_size
    }

    fn layout_sized_box(&mut self, id: LayoutNodeId, constraints: &Constraints) -> Size {
        let width = self.get(id).and_then(|n| n.width);
        let height = self.get(id).and_then(|n| n.height);

        let child_ids = self.children_of(id);
        let child_constraints = Constraints {
            min_width: width.unwrap_or(constraints.min_width),
            max_width: width.unwrap_or(constraints.max_width),
            min_height: height.unwrap_or(constraints.min_height),
            max_height: height.unwrap_or(constraints.max_height),
        };

        if let Some(&child_id) = child_ids.first() {
            self.layout_node(child_id, &child_constraints)
        } else {
            child_constraints.constrain(Size::ZERO)
        }
    }

    fn layout_container(&mut self, id: LayoutNodeId, constraints: &Constraints) -> Size {
        let padding = self.get(id).and_then(|n| n.padding);

        let child_ids = self.children_of(id);
        let inner_constraints = match padding {
            Some(p) => constraints.deflate(p),
            None => *constraints,
        };

        let child_size = if let Some(&child_id) = child_ids.first() {
            self.layout_node(child_id, &inner_constraints)
        } else {
            inner_constraints.constrain(Size::ZERO)
        };

        match padding {
            Some(p) => p.inflate_size(child_size),
            None => child_size,
        }
    }

    fn layout_text(&mut self, id: LayoutNodeId, constraints: &Constraints) -> Size {
        let content = self
            .get(id)
            .and_then(|n| n.text_content.as_deref())
            .unwrap_or("");
        let font_size = self.get(id).and_then(|n| n.font_size).unwrap_or(14.0);

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
}
