use crate::core::layout::{Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::FragmentElement;

// FragmentElement is a transparent pass-through with multiple children: it relays
// the incoming constraints to every child, sizes itself to the union (max)
// of the child sizes, positions all children at the origin, and paints
// nothing itself — it only forwards to children.

impl ElementLayout for FragmentElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let mut max_w = 0.0f64;
        let mut max_h = 0.0f64;
        for &child_id in children {
            let s = cx.layout_child(child_id, constraints);
            if s.width > max_w {
                max_w = s.width;
            }
            if s.height > max_h {
                max_h = s.height;
            }
        }
        let size = constraints.constrain(Size::new(max_w, max_h));

        // --- position (all children at origin) ---
        for &child_id in children {
            cx.set_child_offset(child_id, Offset::ZERO);
        }

        size
    }
}
