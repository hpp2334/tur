use tur_shared::{
    Axis, ComputedLayout, Constraints, CrossAxisAlignment, ElementKind, MainAxisAlignment, Offset,
    Size,
};

use crate::render_object::{ChildLayout, ChildPaint, PaintContext, RenderObject};
use crate::RenderNodeId;

#[derive(Debug)]
struct ChildData {
    id: RenderNodeId,
    size: Size,
    is_flex: bool,
}

#[derive(Debug)]
pub struct FlexRenderObject {
    pub direction: Axis,
    pub main_alignment: MainAxisAlignment,
    pub cross_alignment: CrossAxisAlignment,
    child_data: Vec<ChildData>,
    constraints: Option<Constraints>,
}

impl FlexRenderObject {
    pub fn from_props(props: &std::collections::HashMap<String, tur_element::PropValue>) -> Self {
        let direction = super::prop_str(props, "direction")
            .and_then(|s| match s {
                "Vertical" => Some(Axis::Vertical),
                "Horizontal" => Some(Axis::Horizontal),
                _ => None,
            })
            .or_else(|| {
                super::prop_f64(props, "direction").and_then(|n| match n as i32 {
                    0 => Some(Axis::Vertical),
                    1 => Some(Axis::Horizontal),
                    _ => None,
                })
            })
            .unwrap_or(Axis::Vertical);

        let main_alignment = super::prop_str(props, "mainAlignment")
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
                super::prop_f64(props, "mainAlignment").and_then(|n| match n as i32 {
                    0 => Some(MainAxisAlignment::Start),
                    1 => Some(MainAxisAlignment::Center),
                    2 => Some(MainAxisAlignment::End),
                    3 => Some(MainAxisAlignment::SpaceBetween),
                    4 => Some(MainAxisAlignment::SpaceAround),
                    5 => Some(MainAxisAlignment::SpaceEvenly),
                    _ => None,
                })
            })
            .unwrap_or(MainAxisAlignment::Start);

        let cross_alignment = super::prop_str(props, "crossAlignment")
            .and_then(|s| match s {
                "start" => Some(CrossAxisAlignment::Start),
                "center" => Some(CrossAxisAlignment::Center),
                "end" => Some(CrossAxisAlignment::End),
                "stretch" => Some(CrossAxisAlignment::Stretch),
                _ => None,
            })
            .or_else(|| {
                super::prop_f64(props, "crossAlignment").and_then(|n| match n as i32 {
                    0 => Some(CrossAxisAlignment::Start),
                    1 => Some(CrossAxisAlignment::Center),
                    2 => Some(CrossAxisAlignment::End),
                    3 => Some(CrossAxisAlignment::Stretch),
                    _ => None,
                })
            })
            .unwrap_or(CrossAxisAlignment::Center);

        FlexRenderObject {
            direction,
            main_alignment,
            cross_alignment,
            child_data: Vec::new(),
            constraints: None,
        }
    }
}

impl RenderObject for FlexRenderObject {
    fn kind(&self) -> ElementKind {
        ElementKind::Flex
    }

    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[RenderNodeId],
        child_layout: &mut dyn ChildLayout,
    ) -> Size {
        self.child_data.clear();
        self.constraints = Some(*constraints);

        let mut total_main: f64 = 0.0;
        let mut max_cross: f64 = 0.0;
        let mut flex_count = 0u32;

        for &child_id in children {
            let is_flex = child_layout.get_child_kind(child_id) == ElementKind::FlexItem;

            if is_flex {
                flex_count += 1;
                self.child_data.push(ChildData {
                    id: child_id,
                    size: Size::ZERO,
                    is_flex: true,
                });
            } else {
                let child_constraints = match self.direction {
                    Axis::Vertical => Constraints {
                        min_width: if self.cross_alignment == CrossAxisAlignment::Stretch {
                            constraints.max_width
                        } else {
                            0.0
                        },
                        max_width: constraints.max_width,
                        min_height: 0.0,
                        max_height: (constraints.max_height - total_main).max(0.0),
                    },
                    Axis::Horizontal => Constraints {
                        min_width: 0.0,
                        max_width: (constraints.max_width - total_main).max(0.0),
                        min_height: if self.cross_alignment == CrossAxisAlignment::Stretch {
                            constraints.max_height
                        } else {
                            0.0
                        },
                        max_height: constraints.max_height,
                    },
                };
                let size = child_layout.layout_child(child_id, &child_constraints);
                total_main += self.direction.main(size);
                max_cross = max_cross.max(self.direction.cross(size));
                self.child_data.push(ChildData {
                    id: child_id,
                    size,
                    is_flex: false,
                });
            }
        }

        let available_main = self
            .direction
            .main(constraints.constrain(Size::new(constraints.max_width, constraints.max_height)));
        let remaining_main = (available_main - total_main).max(0.0);
        let flex_space = if flex_count > 0 {
            remaining_main / flex_count as f64
        } else {
            0.0
        };

        for entry in &mut self.child_data {
            if entry.is_flex {
                let child_constraints = match self.direction {
                    Axis::Vertical => Constraints {
                        min_width: if self.cross_alignment == CrossAxisAlignment::Stretch {
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
                        min_height: if self.cross_alignment == CrossAxisAlignment::Stretch {
                            constraints.max_height
                        } else {
                            0.0
                        },
                        max_height: constraints.max_height,
                    },
                };
                let size = child_layout.layout_child(entry.id, &child_constraints);
                entry.size = size;
                max_cross = max_cross.max(self.direction.cross(size));
            }
        }

        let total_main: f64 = self
            .child_data
            .iter()
            .map(|d| self.direction.main(d.size))
            .sum();

        let size = match self.direction {
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

    fn perform_layout_position(
        &mut self,
        _children: &[RenderNodeId],
        child_layout: &mut dyn ChildLayout,
    ) {
        if self.child_data.is_empty() {
            return;
        }

        let allocated_main: f64 = self
            .child_data
            .iter()
            .map(|d| self.direction.main(d.size))
            .sum();

        let constraints = self.constraints.unwrap_or(Constraints::NONE);
        let container_size =
            constraints.constrain(Size::new(constraints.max_width, constraints.max_height));
        let available_main = self.direction.main(container_size);

        let mut current_main = match self.main_alignment {
            MainAxisAlignment::Start | MainAxisAlignment::SpaceBetween => 0.0,
            MainAxisAlignment::Center => (available_main - allocated_main) / 2.0,
            MainAxisAlignment::End => available_main - allocated_main,
            MainAxisAlignment::SpaceAround => {
                (available_main - allocated_main) / (self.child_data.len() as f64 * 2.0)
            }
            MainAxisAlignment::SpaceEvenly => {
                (available_main - allocated_main) / ((self.child_data.len() + 1) as f64)
            }
        };

        let gap = match self.main_alignment {
            MainAxisAlignment::SpaceBetween if self.child_data.len() > 1 => {
                (available_main - allocated_main) / ((self.child_data.len() - 1) as f64)
            }
            MainAxisAlignment::SpaceAround => {
                (available_main - allocated_main) / (self.child_data.len() as f64 * 2.0) * 2.0
            }
            MainAxisAlignment::SpaceEvenly => {
                (available_main - allocated_main) / ((self.child_data.len() + 1) as f64)
            }
            _ => 0.0,
        };

        let container_cross = self.direction.cross(container_size);

        for (i, entry) in self.child_data.iter().enumerate() {
            let cross = match self.cross_alignment {
                CrossAxisAlignment::Start => 0.0,
                CrossAxisAlignment::Center => {
                    (container_cross - self.direction.cross(entry.size)) / 2.0
                }
                CrossAxisAlignment::End => container_cross - self.direction.cross(entry.size),
                CrossAxisAlignment::Stretch => 0.0,
            };

            let offset = match self.direction {
                Axis::Vertical => Offset::new(cross, current_main),
                Axis::Horizontal => Offset::new(current_main, cross),
            };

            child_layout.set_child_offset(entry.id, offset);

            current_main += self.direction.main(entry.size);
            if i < self.child_data.len() - 1 {
                current_main += gap;
            }
        }
    }

    fn paint(
        &self,
        _ctx: &mut dyn PaintContext,
        _offset: Offset,
        _layout: &ComputedLayout,
        children: &[RenderNodeId],
        child_paint: &mut dyn ChildPaint,
    ) {
        for &child_id in children {
            child_paint.paint_child(child_id, _ctx, _offset);
        }
    }
}
