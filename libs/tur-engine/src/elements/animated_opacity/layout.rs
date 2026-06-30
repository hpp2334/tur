use tur_shared::{Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::{lerp_f32, AnimatedOpacityElement};

impl ElementLayout for AnimatedOpacityElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let t = self.host.eased_t();
        self.painting = self.p_value.evaluate(t, lerp_f32).unwrap_or(1.0);
        let size = if let Some(&child_id) = children.first() {
            cx.layout_child(child_id, constraints)
        } else {
            constraints.constrain(Size::ZERO)
        };
        if let Some(&child_id) = children.first() {
            cx.set_child_offset(child_id, Offset::ZERO);
        }
        size
    }
}
