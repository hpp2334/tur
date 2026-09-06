use crate::core::layout::{Constraints, Size, StackFit};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::StackElement;

impl ElementLayout for StackElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let fit = cx
            .read_val_opt(self.view.fit.as_ref())
            .unwrap_or(StackFit::Loose);

        let mut max_size = Size::ZERO;

        for &child_id in children {
            let child_constraints = match fit {
                // `Expand` builds tight constraints from the incoming maxes —
                // under an unbounded axis (e.g. a Stack that is a non-flex
                // child of a Column) that would mint `min = max = ∞` and
                // leak infinite sizes. Degrade that axis to loose instead
                // (Flutter reports this as a layout error) and say so once.
                StackFit::Expand => {
                    let degraded =
                        !constraints.max_width.is_finite() || !constraints.max_height.is_finite();
                    if degraded && !self.warned_expand_unbounded {
                        self.warned_expand_unbounded = true;
                        tracing::error!(
                            "Stack with fit=Expand has unbounded constraints \
                             (width: {}, height: {}): Expand degrades to loose \
                             on the unbounded axes. Give the Stack a bounded \
                             size (e.g. wrap in Expanded) — Flutter reports \
                             this as a layout error.",
                            if constraints.max_width.is_finite() {
                                "bounded"
                            } else {
                                "unbounded"
                            },
                            if constraints.max_height.is_finite() {
                                "bounded"
                            } else {
                                "unbounded"
                            },
                        );
                    }
                    let base = constraints
                        .constrain(Size::new(constraints.max_width, constraints.max_height));
                    Constraints {
                        min_width: if base.width.is_finite() {
                            base.width
                        } else {
                            0.0
                        },
                        max_width: constraints.max_width,
                        min_height: if base.height.is_finite() {
                            base.height
                        } else {
                            0.0
                        },
                        max_height: constraints.max_height,
                    }
                }
                StackFit::Loose => Constraints::loose(
                    constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
                ),
                StackFit::Passthrough => *constraints,
            };
            let size = cx.layout_child(child_id, &child_constraints);
            max_size = Size::new(
                max_size.width.max(size.width),
                max_size.height.max(size.height),
            );
        }

        let final_size = constraints.constrain(max_size);
        self.computed_size = Some(final_size);

        // --- position (assign non-positioned child offsets) ---
        let stack_size = self.computed_size.unwrap_or(Size::ZERO);
        let alignment = cx
            .read_val_opt(self.view.alignment.as_ref())
            .unwrap_or_default();
        for &child_id in children {
            let kind = cx.child_type_name(child_id);
            let is_positioned = kind == "tur_positioned";

            if !is_positioned {
                let child_size = cx.child_computed_size(child_id);
                let offset = alignment.align_offset(stack_size, child_size);
                cx.set_child_offset(child_id, offset);
            }
        }

        final_size
    }
}
