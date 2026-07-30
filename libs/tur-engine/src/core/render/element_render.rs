use vello_common::kurbo::Affine;

use crate::core::layout::{ComputedLayout, Offset};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, PaintContext};

pub trait ElementRender: 'static {
    fn type_name(&self) -> &'static str;

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
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

    /// The paint-only affine this element contributes to its subtree (e.g. a
    /// `Transform` element's rotate/scale/translate), or `None` for plain
    /// elements. Used by compositing consumers (e.g.
    /// `CompositedTransformFollower`) to compose an element's full world
    /// transform from its ancestor chain. Defaults to `None`.
    ///
    /// The affine must be computed from already-resolved paint props (filled
    /// during `perform_layout`) and the node's laid-out `size`; both are
    /// available at paint / compositing time.
    fn paint_transform(&self, _layout: &ComputedLayout) -> Option<Affine> {
        None
    }
}
