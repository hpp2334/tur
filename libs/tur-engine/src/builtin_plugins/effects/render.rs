use vello_common::kurbo::Affine;

use crate::core::element::ElementNodeId;
use crate::core::layout::{ComputedLayout, Size};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::{OpacityElement, TransformElement, TransformPainting};

impl ElementRender for OpacityElement {
    fn type_name(&self) -> &'static str {
        "tur_opacity"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        let opacity: f32 = self.painting.value;
        canvas.push_opacity(opacity);
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas);
        }
        canvas.pop_opacity();
    }
}

impl TransformElement {
    /// Build the affine transform for this element from its resolved paint
    /// props and the child's laid-out `size`.
    ///
    /// `rotate` and `scale` pivot around the point selected by `alignment`
    /// (default: the child center, matching Flutter's `Transform`). The pivot
    /// is expressed inside the child box as `alignment.align_offset(size, 0)`.
    /// `translateX/Y` are plain outer shifts applied to the already-pivoted
    /// element (they are not themselves pivoted).
    pub(crate) fn transform_matrix(p: &TransformPainting, size: Size) -> Affine {
        let sx = p.scale_x.or(p.scale).unwrap_or(1.0);
        let sy = p.scale_y.or(p.scale).unwrap_or(1.0);
        let angle = p.rotate.unwrap_or(0.0);
        let tx = p.translate_x.unwrap_or(0.0);
        let ty = p.translate_y.unwrap_or(0.0);

        let pivot = p.alignment.align_offset(size, Size::ZERO);

        Affine::translate((tx, ty))
            * Affine::translate((pivot.x, pivot.y))
            * Affine::rotate(angle)
            * Affine::scale_non_uniform(sx, sy)
            * Affine::translate((-pivot.x, -pivot.y))
    }
}

impl ElementRender for TransformElement {
    fn type_name(&self) -> &'static str {
        "tur_transform"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        // The rotate/scale/translate matrix is exposed via
        // `relative_transform`; the paint walk pushes it onto the canvas
        // transform stack, so children already paint in the transformed
        // space — no manual `push_transform` here.
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas);
        }
    }

    /// The element's full transform relative to its parent: the layout
    /// translation composed with the resolved rotate/scale/translate matrix.
    /// Paint, hit-test, and bounds all consult this (see `relative_transform`).
    fn relative_transform(&self, layout: &ComputedLayout) -> Affine {
        Affine::translate((layout.offset.x, layout.offset.y))
            * Self::transform_matrix(&self.painting, layout.size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::{Alignment, Size};
    use vello_common::kurbo::Point;

    fn approx(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    /// With no transform set, the matrix is the identity regardless of size.
    #[test]
    fn identity_when_no_props() {
        let p = TransformPainting {
            alignment: Alignment::Center,
            ..Default::default()
        };
        let m = TransformElement::transform_matrix(&p, Size::new(10.0, 10.0));
        assert!(approx(m * Point::new(0.0, 0.0), Point::new(0.0, 0.0)));
        assert!(approx(m * Point::new(10.0, 10.0), Point::new(10.0, 10.0)));
    }

    /// Rotation must pivot around the center: the center is invariant and the
    /// four edge-midpoints permute (a 90° turn moves each off its original spot).
    #[test]
    fn rotate_pivots_around_center() {
        let p = TransformPainting {
            alignment: Alignment::Center,
            rotate: Some(std::f64::consts::FRAC_PI_2),
            ..Default::default()
        };
        let m = TransformElement::transform_matrix(&p, Size::new(10.0, 10.0));

        // Center stays put.
        assert!(approx(m * Point::new(5.0, 5.0), Point::new(5.0, 5.0)));

        // The right-middle point must land on one of the four edge-midpoints
        // (a 90° rotation permutes them) and must have actually moved.
        let mapped = m * Point::new(10.0, 5.0);
        let mids = [
            Point::new(5.0, 0.0),
            Point::new(10.0, 5.0),
            Point::new(5.0, 10.0),
            Point::new(0.0, 5.0),
        ];
        assert!(
            mids.iter().any(|q| approx(mapped, *q)),
            "rotated point {mapped:?} should be an edge-midpoint"
        );
        assert!(
            !approx(mapped, Point::new(10.0, 5.0)),
            "point should have moved"
        );
    }

    /// Scale must pivot around the center: the center is invariant and the
    /// corner moves outward along the diagonal.
    #[test]
    fn scale_pivots_around_center() {
        let p = TransformPainting {
            alignment: Alignment::Center,
            scale: Some(2.0),
            ..Default::default()
        };
        let m = TransformElement::transform_matrix(&p, Size::new(10.0, 10.0));
        // Center fixed.
        assert!(approx(m * Point::new(5.0, 5.0), Point::new(5.0, 5.0)));
        // Top-left corner (0,0) → (-5,-5): (0-5)*2 + 5.
        assert!(approx(m * Point::new(0.0, 0.0), Point::new(-5.0, -5.0)));
        // Bottom-right corner (10,10) → (15,15).
        assert!(approx(m * Point::new(10.0, 10.0), Point::new(15.0, 15.0)));
    }

    /// `alignment: TopLeft` restores the legacy pivot (corner): a rotation
    /// leaves the top-left corner pinned at the origin.
    #[test]
    fn top_left_alignment_pivots_at_corner() {
        let p = TransformPainting {
            alignment: Alignment::TopLeft,
            rotate: Some(std::f64::consts::FRAC_PI_2),
            ..Default::default()
        };
        let m = TransformElement::transform_matrix(&p, Size::new(10.0, 10.0));
        // Corner is the pivot → invariant.
        assert!(approx(m * Point::new(0.0, 0.0), Point::new(0.0, 0.0)));
        // The center (5,5) is no longer fixed.
        assert!(
            !approx(m * Point::new(5.0, 5.0), Point::new(5.0, 5.0)),
            "center must move when pivoting at the corner"
        );
    }
}
