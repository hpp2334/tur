use tur_shared::{
    Axis, ComputedLayout, Constraints, CrossAxisAlignment, MainAxisSize, MainAxisAlignment,
    Offset, Size,
};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::{ChildData, FlexElement};

impl ElementLayout for FlexElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let direction = self.component.direction.unwrap_or(Axis::Vertical);
        let cross_alignment = cx
            .read_val_opt(self.component.cross_alignment.as_ref())
            .unwrap_or(CrossAxisAlignment::Center);
        let main_axis_size = cx
            .read_val_opt(self.component.main_axis_size.as_ref())
            .unwrap_or(MainAxisSize::Max);

        self.child_data.clear();
        self.constraints = Some(*constraints);

        let mut total_main: f64 = 0.0;
        let mut max_cross: f64 = 0.0;
        let mut total_flex: f64 = 0.0;

        for &child_id in children {
            let is_flex = cx.child_type_name(child_id) == "tur_flex_item";

            if is_flex {
                let flex = cx.child_flex(child_id).max(0.0);
                total_flex += flex;
                self.child_data.push(ChildData {
                    id: child_id,
                    size: Size::ZERO,
                    is_flex: true,
                    flex,
                });
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
                        max_height: (constraints.max_height - total_main).max(0.0),
                    },
                    Axis::Horizontal => Constraints {
                        min_width: 0.0,
                        max_width: (constraints.max_width - total_main).max(0.0),
                        min_height: if cross_alignment == CrossAxisAlignment::Stretch {
                            constraints.max_height
                        } else {
                            0.0
                        },
                        max_height: constraints.max_height,
                    },
                };
                let size = cx.layout_child(child_id, &child_constraints);
                total_main += direction.main(size);
                max_cross = max_cross.max(direction.cross(size));
                self.child_data.push(ChildData {
                    id: child_id,
                    size,
                    is_flex: false,
                    flex: 0.0,
                });
            }
        }

        let available_main = direction.main(
            constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
        );
        let remaining_main = (available_main - total_main).max(0.0);
        let space_per_unit = if total_flex > 0.0 {
            remaining_main / total_flex
        } else {
            0.0
        };

        for entry in &mut self.child_data {
            if entry.is_flex {
                let slot = space_per_unit * entry.flex;
                let child_constraints = match direction {
                    Axis::Vertical => Constraints {
                        min_width: if cross_alignment == CrossAxisAlignment::Stretch {
                            constraints.max_width
                        } else {
                            0.0
                        },
                        max_width: constraints.max_width,
                        min_height: slot,
                        max_height: slot,
                    },
                    Axis::Horizontal => Constraints {
                        min_width: slot,
                        max_width: slot,
                        min_height: if cross_alignment == CrossAxisAlignment::Stretch {
                            constraints.max_height
                        } else {
                            0.0
                        },
                        max_height: constraints.max_height,
                    },
                };
                let size = cx.layout_child(entry.id, &child_constraints);
                entry.size = size;
                max_cross = max_cross.max(direction.cross(size));
            }
        }

        let total_main: f64 = self.child_data.iter().map(|d| direction.main(d.size)).sum();

        let main_size = match main_axis_size {
            MainAxisSize::Max => {
                let max_main = match direction {
                    Axis::Vertical => constraints.max_height,
                    Axis::Horizontal => constraints.max_width,
                };
                if max_main.is_finite() { max_main } else { total_main }
            }
            MainAxisSize::Min => total_main,
        };

        let size = match direction {
            Axis::Vertical => Size::new(
                max_cross.clamp(constraints.min_width, constraints.max_width),
                main_size.clamp(constraints.min_height, constraints.max_height),
            ),
            Axis::Horizontal => Size::new(
                main_size.clamp(constraints.min_width, constraints.max_width),
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

        let direction = self.component.direction.unwrap_or(Axis::Vertical);
        let main_alignment = cx
            .read_val_opt(self.component.main_alignment.as_ref())
            .unwrap_or(MainAxisAlignment::Start);
        let cross_alignment = cx
            .read_val_opt(self.component.cross_alignment.as_ref())
            .unwrap_or(CrossAxisAlignment::Center);

        let allocated_main: f64 = self.child_data.iter().map(|d| direction.main(d.size)).sum();

        let container_size = self.computed_size.unwrap_or_else(|| {
            let constraints = self.constraints.unwrap_or(Constraints::NONE);
            constraints.constrain(Size::new(constraints.max_width, constraints.max_height))
        });
        let available_main = direction.main(container_size);

        let mut current_main = match main_alignment {
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

        let gap = match main_alignment {
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

        let container_cross = direction.cross(container_size);

        for (i, entry) in self.child_data.iter().enumerate() {
            let cross = match cross_alignment {
                CrossAxisAlignment::Start => 0.0,
                CrossAxisAlignment::Center => (container_cross - direction.cross(entry.size)) / 2.0,
                CrossAxisAlignment::End => container_cross - direction.cross(entry.size),
                CrossAxisAlignment::Stretch => 0.0,
            };

            let offset = match direction {
                Axis::Vertical => Offset::new(cross, current_main),
                Axis::Horizontal => Offset::new(current_main, cross),
            };

            cx.set_child_offset(entry.id, offset);

            current_main += direction.main(entry.size);
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
