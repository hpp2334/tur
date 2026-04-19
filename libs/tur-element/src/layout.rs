use tur_shared::{
    Axis, ComputedLayout, Constraints, CrossAxisAlignment, EdgeInsets, MainAxisAlignment, Offset,
    Size, StackFit,
};

use crate::{ElementKind, ElementNodeId, ElementTree};

pub struct LayoutResult {
    pub size: Size,
}

pub fn compute_layout(element_tree: &mut ElementTree, constraints: &Constraints) -> LayoutResult {
    let root_id = match element_tree.root_id() {
        Some(id) => id,
        None => {
            return LayoutResult {
                size: constraints.constrain(Size::ZERO),
            }
        }
    };

    clear_computed_layouts(element_tree);

    let size = layout_node(element_tree, root_id, constraints);
    LayoutResult { size }
}

fn clear_computed_layouts(element_tree: &mut ElementTree) {
    if let Some(root_id) = element_tree.root_id() {
        clear_subtree(element_tree, root_id);
    }
}

fn clear_subtree(element_tree: &mut ElementTree, id: ElementNodeId) {
    let children = element_tree.children_of(id);
    element_tree.get_mut(id).unwrap().computed_layout = None;
    for child_id in children {
        clear_subtree(element_tree, child_id);
    }
}

fn layout_node(
    element_tree: &mut ElementTree,
    id: ElementNodeId,
    constraints: &Constraints,
) -> Size {
    let kind = match element_tree.get(id) {
        Some(n) => n.kind,
        None => return constraints.constrain(Size::ZERO),
    };

    let size = match kind {
        ElementKind::Flex => {
            let direction = parse_direction(element_tree, id);
            layout_flex(element_tree, id, constraints, direction)
        }
        ElementKind::FlexItem => layout_flex_item(element_tree, id, constraints),
        ElementKind::Stack => layout_stack(element_tree, id, constraints),
        ElementKind::Positioned => layout_positioned(element_tree, id, constraints),
        ElementKind::Container => layout_container(element_tree, id, constraints),
        ElementKind::Text => layout_text(element_tree, id, constraints),
    };

    let constrained = constraints.constrain(size);
    let offset = element_tree
        .get(id)
        .and_then(|n| n.computed_layout)
        .map_or(Offset::ZERO, |cl| cl.offset);
    element_tree.get_mut(id).unwrap().computed_layout = Some(ComputedLayout {
        size: constrained,
        offset,
    });
    constrained
}

fn parse_main_alignment(element_tree: &ElementTree, id: ElementNodeId) -> MainAxisAlignment {
    let node = match element_tree.get(id) {
        Some(n) => n,
        None => return MainAxisAlignment::Start,
    };

    node.prop_str("mainAlignment")
        .and_then(|s| match s {
            "start" => Some(MainAxisAlignment::Start),
            "center" => Some(MainAxisAlignment::Center),
            "end" => Some(MainAxisAlignment::End),
            "space-between" => Some(MainAxisAlignment::SpaceBetween),
            "space-around" => Some(MainAxisAlignment::SpaceAround),
            "space-evenly" => Some(MainAxisAlignment::SpaceEvenly),
            _ => None,
        })
        .or_else(|| {
            node.prop_f64("mainAlignment").and_then(|n| match n as i32 {
                0 => Some(MainAxisAlignment::Start),
                1 => Some(MainAxisAlignment::Center),
                2 => Some(MainAxisAlignment::End),
                3 => Some(MainAxisAlignment::SpaceBetween),
                4 => Some(MainAxisAlignment::SpaceAround),
                5 => Some(MainAxisAlignment::SpaceEvenly),
                _ => None,
            })
        })
        .unwrap_or(MainAxisAlignment::Start)
}

fn parse_cross_alignment(element_tree: &ElementTree, id: ElementNodeId) -> CrossAxisAlignment {
    let node = match element_tree.get(id) {
        Some(n) => n,
        None => return CrossAxisAlignment::Center,
    };

    node.prop_str("crossAlignment")
        .and_then(|s| match s {
            "start" => Some(CrossAxisAlignment::Start),
            "center" => Some(CrossAxisAlignment::Center),
            "end" => Some(CrossAxisAlignment::End),
            "stretch" => Some(CrossAxisAlignment::Stretch),
            _ => None,
        })
        .or_else(|| {
            node.prop_f64("crossAlignment")
                .and_then(|n| match n as i32 {
                    0 => Some(CrossAxisAlignment::Start),
                    1 => Some(CrossAxisAlignment::Center),
                    2 => Some(CrossAxisAlignment::End),
                    3 => Some(CrossAxisAlignment::Stretch),
                    _ => None,
                })
        })
        .unwrap_or(CrossAxisAlignment::Center)
}

fn parse_stack_fit(element_tree: &ElementTree, id: ElementNodeId) -> StackFit {
    let node = match element_tree.get(id) {
        Some(n) => n,
        None => return StackFit::Loose,
    };

    node.prop_str("fit")
        .and_then(|s| match s {
            "loose" => Some(StackFit::Loose),
            "expand" => Some(StackFit::Expand),
            "passthrough" => Some(StackFit::Passthrough),
            _ => None,
        })
        .or_else(|| {
            node.prop_f64("fit").and_then(|n| match n as i32 {
                0 => Some(StackFit::Loose),
                1 => Some(StackFit::Expand),
                2 => Some(StackFit::Passthrough),
                _ => None,
            })
        })
        .unwrap_or(StackFit::Loose)
}

fn parse_direction(element_tree: &ElementTree, id: ElementNodeId) -> Axis {
    let node = match element_tree.get(id) {
        Some(n) => n,
        None => return Axis::Vertical,
    };

    node.prop_str("direction")
        .and_then(|s| match s {
            "Vertical" => Some(Axis::Vertical),
            "Horizontal" => Some(Axis::Horizontal),
            _ => None,
        })
        .or_else(|| {
            node.prop_f64("direction").and_then(|n| match n as i32 {
                0 => Some(Axis::Vertical),
                1 => Some(Axis::Horizontal),
                _ => None,
            })
        })
        .unwrap_or(Axis::Vertical)
}

fn layout_flex(
    element_tree: &mut ElementTree,
    id: ElementNodeId,
    constraints: &Constraints,
    direction: Axis,
) -> Size {
    let child_ids = element_tree.children_of(id);

    let main_alignment = parse_main_alignment(element_tree, id);
    let cross_alignment = parse_cross_alignment(element_tree, id);

    let (mut total_main, mut max_cross): (f64, f64) = (0.0, 0.0);
    let mut flex_count = 0u32;
    let mut child_sizes: Vec<(ElementNodeId, Size, bool)> = Vec::with_capacity(child_ids.len());

    for &child_id in &child_ids {
        let is_flex = element_tree
            .get(child_id)
            .map(|n| n.kind == ElementKind::FlexItem)
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
            let size = layout_node(element_tree, child_id, &child_constraints);
            total_main += direction.main(size);
            max_cross = max_cross.max(direction.cross(size));
            child_sizes.push((child_id, size, false));
        }
    }

    let available_main = direction
        .main(constraints.constrain(Size::new(constraints.max_width, constraints.max_height)));
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
            let size = layout_node(element_tree, child_id, &child_constraints);
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
                (direction.cross(
                    constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
                ) - direction.cross(*child_size))
                    / 2.0
            }
            CrossAxisAlignment::End => {
                direction.cross(
                    constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
                ) - direction.cross(*child_size)
            }
            CrossAxisAlignment::Stretch => 0.0,
        };

        let offset = match direction {
            Axis::Vertical => Offset::new(cross, current_main),
            Axis::Horizontal => Offset::new(current_main, cross),
        };

        let child = element_tree.get_mut(*child_id).unwrap();
        if let Some(ref mut cl) = child.computed_layout {
            cl.offset = offset;
        } else {
            child.computed_layout = Some(ComputedLayout {
                size: *child_size,
                offset,
            });
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

fn layout_flex_item(
    element_tree: &mut ElementTree,
    id: ElementNodeId,
    constraints: &Constraints,
) -> Size {
    let child_ids = element_tree.children_of(id);
    if let Some(&child_id) = child_ids.first() {
        layout_node(element_tree, child_id, constraints)
    } else {
        constraints.constrain(Size::ZERO)
    }
}

fn layout_stack(
    element_tree: &mut ElementTree,
    id: ElementNodeId,
    constraints: &Constraints,
) -> Size {
    let child_ids = element_tree.children_of(id);
    let stack_fit = parse_stack_fit(element_tree, id);

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
        let size = layout_node(element_tree, child_id, &child_constraints);
        max_size = Size::new(
            max_size.width.max(size.width),
            max_size.height.max(size.height),
        );
    }

    let stack_size = constraints.constrain(max_size);

    for &child_id in &child_ids {
        let is_positioned = element_tree
            .get(child_id)
            .map(|n| n.kind == ElementKind::Positioned)
            .unwrap_or(false);

        if !is_positioned {
            let child = element_tree.get_mut(child_id).unwrap();
            if let Some(ref mut cl) = child.computed_layout {
                cl.offset = Offset::ZERO;
            }
        }
    }

    stack_size
}

fn layout_positioned(
    element_tree: &mut ElementTree,
    id: ElementNodeId,
    constraints: &Constraints,
) -> Size {
    let child_ids = element_tree.children_of(id);

    let left = element_tree.get(id).and_then(|n| n.prop_f64("left"));
    let top = element_tree.get(id).and_then(|n| n.prop_f64("top"));
    let right = element_tree.get(id).and_then(|n| n.prop_f64("right"));
    let bottom = element_tree.get(id).and_then(|n| n.prop_f64("bottom"));

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
        layout_node(element_tree, child_id, &child_constraints)
    } else {
        child_constraints.constrain(Size::ZERO)
    };

    let offset_x = left.unwrap_or(0.0);
    let offset_y = top.unwrap_or(0.0);

    element_tree.get_mut(id).unwrap().computed_layout = Some(ComputedLayout {
        size: child_size,
        offset: Offset::new(offset_x, offset_y),
    });

    child_size
}

fn layout_container(
    element_tree: &mut ElementTree,
    id: ElementNodeId,
    constraints: &Constraints,
) -> Size {
    let node = element_tree.get(id).unwrap();
    let width = node.prop_f64("width");
    let height = node.prop_f64("height");
    let padding = node.prop_f64("padding").map(EdgeInsets::all);

    let child_ids = element_tree.children_of(id);

    let sized_constraints = Constraints {
        min_width: width.unwrap_or(constraints.min_width),
        max_width: width.unwrap_or(constraints.max_width),
        min_height: height.unwrap_or(constraints.min_height),
        max_height: height.unwrap_or(constraints.max_height),
    };

    let inner_constraints = match padding {
        Some(p) => sized_constraints.deflate(p),
        None => sized_constraints,
    };

    let child_size = if let Some(&child_id) = child_ids.first() {
        layout_node(element_tree, child_id, &inner_constraints)
    } else {
        inner_constraints.constrain(Size::ZERO)
    };

    let inflated = match padding {
        Some(p) => p.inflate_size(child_size),
        None => child_size,
    };

    sized_constraints.constrain(inflated)
}

fn layout_text(
    element_tree: &mut ElementTree,
    id: ElementNodeId,
    constraints: &Constraints,
) -> Size {
    let content = element_tree
        .get(id)
        .and_then(|n| n.prop_str("content"))
        .unwrap_or("");
    let font_size = element_tree
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
