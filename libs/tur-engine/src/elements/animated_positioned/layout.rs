use tur_shared::{Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::{lerp_f64, AnimatedPositionedElement};

impl ElementLayout for AnimatedPositionedElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let t = self.host.eased_t();
        let left = self.p_left.evaluate(t, lerp_f64);
        let top = self.p_top.evaluate(t, lerp_f64);
        let right = self.p_right.evaluate(t, lerp_f64);
        let bottom = self.p_bottom.evaluate(t, lerp_f64);
        let width = self.p_width.evaluate(t, lerp_f64);
        let height = self.p_height.evaluate(t, lerp_f64);

        // Same independent-axis sizing model as PositionedElement, using the
        // animated values.
        let tight_w = width.or_else(|| match (left, right) {
            (Some(l), Some(r)) => Some((constraints.max_width - l - r).max(0.0)),
            _ => None,
        });
        let tight_h = height.or_else(|| match (top, bottom) {
            (Some(t), Some(b)) => Some((constraints.max_height - t - b).max(0.0)),
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
        let offset_x = left.unwrap_or(0.0);
        let offset_y = top.unwrap_or(0.0);
        cx.set_child_offset_self(Offset::new(offset_x, offset_y));

        size
    }
}
