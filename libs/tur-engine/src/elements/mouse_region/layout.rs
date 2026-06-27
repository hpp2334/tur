use tur_shared::{Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::MouseRegionElement;

// MouseRegionElement is a transparent pass-through: it relays constraints to
// its single child, takes the child's size, and positions the child at the
// origin. It paints nothing itself.

impl ElementLayout for MouseRegionElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let size = if let Some(&child_id) = children.first() {
            cx.layout_child(child_id, constraints)
        } else {
            constraints.constrain(Size::ZERO)
        };

        // --- position (child at origin) ---
        if let Some(&child_id) = children.first() {
            cx.set_child_offset(child_id, Offset::ZERO);
        }

        // Resolve the reactive cursor prop during layout (the one phase with a
        // JS engine available); the pointer handler reads `self.cursor` later.
        self.cursor = cx.read_val_opt(self.component.cursor.as_ref());

        size
    }
}
