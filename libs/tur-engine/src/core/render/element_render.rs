use vello_common::kurbo::Affine;

use crate::core::layout::{ComputedLayout, Offset};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, PaintContext};

pub trait ElementRender: 'static {
    fn type_name(&self) -> &'static str;

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    );

    fn hit_test(&self, position: Offset, layout: &ComputedLayout) -> bool {
        position.x >= 0.0
            && position.x < layout.size.width
            && position.y >= 0.0
            && position.y < layout.size.height
    }

    /// This element's transform **relative to its parent** — the affine that
    /// maps the element's local space into its parent's space. The paint walk
    /// pushes this onto the canvas transform stack; hit-testing maps the
    /// pointer through its inverse; bounds compose it down the chain into the
    /// node's absolute (world) transform (`absolute_affine_of`).
    ///
    /// Default: a pure translation by `layout.offset` (the position layout
    /// assigned). `Transform` overrides this to fold in its rotate/scale;
    /// `CompositedTransformFollower` overrides it to translate by its
    /// link-tracked offset (ignoring the layout offset). Because paint,
    /// hit-test, and bounds all consult this one hook, an element's painted
    /// position, its hit region, and its reported bounds always agree.
    ///
    /// Must be computed from already-resolved props (filled during
    /// `perform_layout`) and the laid-out `size`.
    fn relative_transform(&self, layout: &ComputedLayout) -> Affine {
        Affine::translate((layout.offset.x, layout.offset.y))
    }
}
