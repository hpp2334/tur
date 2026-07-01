use tur_shared::{Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::ReadableSubscribeElement;

// Transparent pass-through: adopt the single child's size and place it at the
// origin.
impl ElementLayout for ReadableSubscribeElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let size = if let Some(child_id) = children.first() {
            cx.layout_child(*child_id, constraints)
        } else {
            constraints.constrain(Size::ZERO)
        };
        if let Some(child_id) = children.first() {
            cx.set_child_offset(*child_id, Offset::ZERO);
        }
        size
    }
}
