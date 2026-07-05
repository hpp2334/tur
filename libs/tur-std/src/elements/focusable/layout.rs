use tur_shared::{Constraints, Size};

use tur_engine::core::element::ElementNodeId;
use tur_engine::core::layout::{ElementLayout, LayoutContext};

use super::element::FocusableElement;

impl ElementLayout for FocusableElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        if let Some(&child_id) = children.first() {
            cx.layout_child(child_id, constraints)
        } else {
            constraints.constrain(Size::ZERO)
        }
    }
}
