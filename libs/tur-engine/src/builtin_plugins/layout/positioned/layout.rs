use crate::core::layout::{Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::PositionedElement;

impl ElementLayout for PositionedElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let left = cx.read_val_opt(self.view.left.as_ref());
        let top = cx.read_val_opt(self.view.top.as_ref());
        let right = cx.read_val_opt(self.view.right.as_ref());
        let bottom = cx.read_val_opt(self.view.bottom.as_ref());
        let width = cx.read_val_opt(self.view.width.as_ref());
        let height = cx.read_val_opt(self.view.height.as_ref());

        // Resolve each axis independently: explicit size wins; otherwise a
        // pair of opposing edges implies a tight extent; else loose. An edge
        // pair against an UNBOUNDED axis max (e.g. a Stack that is a non-flex
        // child of a Column) must not mint a tight-∞ extent — degrade that
        // axis to loose instead so no infinite size leaks upward (Flutter
        // reports this as a layout error; we also log it once).
        let edge_w_degraded = left.is_some()
            && right.is_some()
            && width.is_none()
            && !constraints.max_width.is_finite();
        let edge_h_degraded = top.is_some()
            && bottom.is_some()
            && height.is_none()
            && !constraints.max_height.is_finite();
        if (edge_w_degraded || edge_h_degraded) && !self.warned_edges_unbounded {
            self.warned_edges_unbounded = true;
            tracing::error!(
                "Positioned with opposing edges ({}{}) inside a Stack with \
                 unbounded constraints: the pair degrades to a loose axis. \
                 Give the Stack a bounded size or set an explicit width/height \
                 — Flutter reports this as a layout error.",
                if edge_w_degraded { "left+right " } else { "" },
                if edge_h_degraded { "top+bottom " } else { "" },
            );
        }
        let tight_w = width.or_else(|| match (left, right) {
            (Some(l), Some(r)) if constraints.max_width.is_finite() => {
                Some((constraints.max_width - l - r).max(0.0))
            }
            _ => None,
        });
        let tight_h = height.or_else(|| match (top, bottom) {
            (Some(t), Some(b)) if constraints.max_height.is_finite() => {
                Some((constraints.max_height - t - b).max(0.0))
            }
            _ => None,
        });

        let child_constraints = match (tight_w, tight_h) {
            (Some(w), Some(h)) => Constraints::tight(Size::new(w, h)),
            (Some(w), None) => Constraints {
                min_width: w,
                max_width: w,
                min_height: 0.0,
                max_height: constraints.max_height,
            },
            (None, Some(h)) => Constraints {
                min_width: 0.0,
                max_width: constraints.max_width,
                min_height: h,
                max_height: h,
            },
            (None, None) => Constraints::loose(
                constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
            ),
        };

        let size = if let Some(&child_id) = children.first() {
            cx.layout_child(child_id, &child_constraints)
        } else {
            child_constraints.constrain(Size::ZERO)
        };

        // --- position (set own offset within the parent Stack) ---
        // Edge anchoring (Flutter `RenderStack` position resolution):
        // `left`/`top` win when set; otherwise `right`/`bottom` anchor from
        // the stack's far edges — `x = stack_w - right - child_w`. The
        // stack-size-bounded constraints make `max_width`/`max_height` equal
        // the stack extent; a `right`/`bottom` anchor under an UNBOUNDED
        // stack axis has no reference box and degrades to 0 (logged once).
        let anchor_unbounded =
            (right.is_some() && left.is_none() && !constraints.max_width.is_finite())
                || (bottom.is_some() && top.is_none() && !constraints.max_height.is_finite());
        if anchor_unbounded && !self.warned_anchor_unbounded {
            self.warned_anchor_unbounded = true;
            tracing::error!(
                "Positioned anchored with {}inside a Stack with unbounded \
                 constraints: the anchor is ignored (no reference size to \
                 resolve against). Give the Stack a bounded size on that \
                 axis — Flutter reports this as a layout error.",
                if right.is_some() && left.is_none() && !constraints.max_width.is_finite() {
                    "right "
                } else {
                    "bottom "
                },
            );
        }
        let offset_x = match (left, right) {
            (Some(l), _) => l,
            (None, Some(r)) if constraints.max_width.is_finite() => {
                constraints.max_width - r - size.width
            }
            _ => 0.0,
        };
        let offset_y = match (top, bottom) {
            (Some(t), _) => t,
            (None, Some(b)) if constraints.max_height.is_finite() => {
                constraints.max_height - b - size.height
            }
            _ => 0.0,
        };
        cx.set_child_offset_self(Offset::new(offset_x, offset_y));

        size
    }
}
