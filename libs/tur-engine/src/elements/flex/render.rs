use tur_shared::{
    Axis, ComputedLayout, Constraints, CrossAxisAlignment, MainAxisAlignment, Offset, Size,
};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::FlexElement;

impl ElementLayout for FlexElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        self.child_data.clear();
        self.constraints = Some(*constraints);

        let mut total_main: f64 = 0.0;
        let mut max_cross: f64 = 0.0;
        let mut flex_count = 0u32;

        for &child_id in children {
            let is_flex = cx.child_type_name(child_id) == "tur_flex_item";

            if is_flex {
                flex_count += 1;
                self.child_data.push(super::element::ChildData {
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
                let size = cx.layout_child(child_id, &child_constraints);
                total_main += self.direction.main(size);
                max_cross = max_cross.max(self.direction.cross(size));
                self.child_data.push(super::element::ChildData {
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
                let size = cx.layout_child(entry.id, &child_constraints);
                entry.size = size;
                max_cross = max_cross.max(self.direction.cross(size));
            }
        }

        let _total_main: f64 = self
            .child_data
            .iter()
            .map(|d| self.direction.main(d.size))
            .sum();

        let size = match self.direction {
            Axis::Vertical => Size::new(
                max_cross.clamp(constraints.min_width, constraints.max_width),
                constraints.max_height.clamp(constraints.min_height, constraints.max_height),
            ),
            Axis::Horizontal => Size::new(
                constraints.max_width.clamp(constraints.min_width, constraints.max_width),
                max_cross.clamp(constraints.min_height, constraints.max_height),
            ),
        };

        let final_size = constraints.constrain(size);
        self.computed_size = Some(final_size);
        final_size
    }

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], cx: &mut LayoutContext) {
        if self.child_data.is_empty() {
            return;
        }

        let allocated_main: f64 = self
            .child_data
            .iter()
            .map(|d| self.direction.main(d.size))
            .sum();

        let container_size = self.computed_size.unwrap_or_else(|| {
            let constraints = self.constraints.unwrap_or(Constraints::NONE);
            constraints.constrain(Size::new(constraints.max_width, constraints.max_height))
        });
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

            cx.set_child_offset(entry.id, offset);

            current_main += self.direction.main(entry.size);
            if i < self.child_data.len() - 1 {
                current_main += gap;
            }
        }
    }
}

impl ElementRender for FlexElement {
    fn type_name(&self) -> &'static str {
        "tur_flex"
    }

    fn paint(
        &self,
        _canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        for &child_id in children {
            paint_ctx.paint_child(child_id, _canvas, offset);
        }
    }
}
