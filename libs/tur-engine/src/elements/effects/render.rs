use tur_shared::{ComputedLayout, Offset};
use vello_common::kurbo::Affine;

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::{OpacityElement, TransformElement};

impl ElementRender for OpacityElement {
    fn type_name(&self) -> &'static str {
        "tur_opacity"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        let opacity: f32 = self.painting.value;
        canvas.push_opacity(opacity);
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas, offset);
        }
        canvas.pop_opacity();
    }
}

impl TransformElement {
    /// Resolve the transform from painting props (filled during layout).
    fn resolve_transform(&self) -> Affine {
        let p = &self.painting;
        let sx = p.scale_x.or(p.scale).unwrap_or(1.0);
        let sy = p.scale_y.or(p.scale).unwrap_or(1.0);
        let angle = p.rotate.unwrap_or(0.0);
        let tx = p.translate_x.unwrap_or(0.0);
        let ty = p.translate_y.unwrap_or(0.0);

        Affine::translate((tx, ty))
            * Affine::rotate(angle)
            * Affine::scale(sx)
            * Affine::scale(sy)
    }
}

impl ElementRender for TransformElement {
    fn type_name(&self) -> &'static str {
        "tur_transform"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        let local = self.resolve_transform();
        // Combine the canvas offset (parent-relative origin) with the local
        // transform so the child paints in the right place.
        let combined = Affine::translate((offset.x, offset.y)) * local;
        canvas.push_transform(combined);
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas, Offset::ZERO);
        }
        canvas.pop_transform();
    }
}
