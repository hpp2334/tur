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

        // Positioned children are laid out in a SECOND pass — after the
        // Stack's own size is known — so their edge anchors (`right`/
        // `bottom`) and opposing-edge extents resolve against the STACK's
        // size rather than the incoming constraints (Flutter `RenderStack`:
        // non-positioned children size the stack, then
        // `constraintsForPositionedChild` bounds each axis to the stack
        // extent).
        let mut non_positioned: Vec<ElementNodeId> = Vec::new();
        let mut positioned: Vec<ElementNodeId> = Vec::new();
        for &child_id in children {
            if cx.child_type_name(child_id) == "tur_positioned" {
                positioned.push(child_id);
            } else {
                non_positioned.push(child_id);
            }
        }

        let mut max_size = Size::ZERO;

        for &child_id in &non_positioned {
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

        // --- stack size ---
        // Sized by its non-positioned children; a positioned-only stack
        // takes the biggest size its constraints allow (Flutter
        // `RenderStack`: `size = constraints.biggest`) — the reference box
        // `right`/`bottom` anchors resolve against. An unbounded axis
        // degrades to the positioned children's max extent (resolved after
        // their pass) instead of leaking `∞` (Flutter reports this as a
        // layout error).
        let (mut stack_w, mut stack_h) = if !non_positioned.is_empty() || positioned.is_empty() {
            let s = constraints.constrain(max_size);
            (s.width, s.height)
        } else {
            let biggest =
                constraints.constrain(Size::new(constraints.max_width, constraints.max_height));
            (biggest.width, biggest.height)
        };

        // --- pass 2: positioned children, bounded per-axis by the stack size ---
        let mut pos_max = Size::ZERO;
        for &child_id in &positioned {
            let child_constraints = Constraints {
                min_width: 0.0,
                max_width: stack_w,
                min_height: 0.0,
                max_height: stack_h,
            };
            let size = cx.layout_child(child_id, &child_constraints);
            pos_max = Size::new(
                pos_max.width.max(size.width),
                pos_max.height.max(size.height),
            );
        }
        if !stack_w.is_finite() {
            stack_w = pos_max.width;
        }
        if !stack_h.is_finite() {
            stack_h = pos_max.height;
        }

        let final_size = Size::new(stack_w, stack_h);
        self.computed_size = Some(final_size);

        // --- position (assign non-positioned child offsets) ---
        let stack_size = self.computed_size.unwrap_or(Size::ZERO);
        let alignment = cx
            .read_val_opt(self.view.alignment.as_ref())
            .unwrap_or_default();
        for &child_id in &non_positioned {
            let child_size = cx.child_computed_size(child_id);
            let offset = alignment.align_offset(stack_size, child_size);
            cx.set_child_offset(child_id, offset);
        }

        final_size
    }
}
