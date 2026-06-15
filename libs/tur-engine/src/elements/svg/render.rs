use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::Svg;

impl ElementLayout for Svg {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let width = cx.read_val_opt(self.spec.width.as_ref());
        let height = cx.read_val_opt(self.spec.height.as_ref());

        let w = width.unwrap_or_else(|| {
            if constraints.max_width.is_finite() {
                constraints.max_width
            } else {
                0.0
            }
        });
        let h = height.unwrap_or_else(|| {
            if constraints.max_height.is_finite() {
                constraints.max_height
            } else {
                0.0
            }
        });

        constraints.constrain(Size::new(w, h))
    }

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], _cx: &mut LayoutContext) {}
}

impl ElementRender for Svg {
    fn type_name(&self) -> &'static str {
        "tur_svg"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        let _ = canvas;
        let _ = offset;
        let _ = paint_ctx
            .read_val_opt(self.spec.fit.as_ref())
            .unwrap_or_default();

        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas, offset);
        }
    }
}
