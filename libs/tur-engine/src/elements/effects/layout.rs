use tur_shared::{Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::{OpacityElement, TransformElement};

impl ElementLayout for OpacityElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        // Resolve the paint-time opacity here (layout holds the store); paint
        // reads `self.painting` and never touches the store.
        self.painting.value = cx.read_val_opt(self.component.value.as_ref()).unwrap_or(1.0);
        let size = if let Some(child_id) = children.first() {
            cx.layout_child(*child_id, constraints)
        } else {
            constraints.constrain(Size::ZERO)
        };

        // --- position ---
        if let Some(child_id) = children.first() {
            cx.set_child_offset(*child_id, Offset::ZERO);
        }

        size
    }
}

impl ElementLayout for TransformElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        // Layout the child untransformed; the transform is applied at paint
        // time only. (For non-uniform scale this means layout uses the
        // untransformed size, which is correct for hit-testing but not for
        // visual bounds. Acceptable for animation effects.)
        let size = if let Some(child_id) = children.first() {
            cx.layout_child(*child_id, constraints)
        } else {
            constraints.constrain(Size::ZERO)
        };

        // Resolve transform paint props here (layout holds the store); paint
        // reads `self.painting` and never touches the store.
        self.painting = super::element::TransformPainting {
            scale: cx.read_val_opt(self.component.scale.as_ref()),
            scale_x: cx.read_val_opt(self.component.scale_x.as_ref()),
            scale_y: cx.read_val_opt(self.component.scale_y.as_ref()),
            rotate: cx.read_val_opt(self.component.rotate.as_ref()),
            translate_x: cx.read_val_opt(self.component.translate_x.as_ref()),
            translate_y: cx.read_val_opt(self.component.translate_y.as_ref()),
        };

        // --- position ---
        if let Some(child_id) = children.first() {
            cx.set_child_offset(*child_id, Offset::ZERO);
        }

        size
    }
}
