use vello_common::kurbo::Affine;

use crate::core::layout::{ComputedLayout, Offset};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, PaintContext};

/// What an element contributes to a hit test **by itself**, at a point
/// already within its bounds. Evaluated only AFTER all children have been
/// tested — mirroring Flutter's `RenderBox.hitTest`:
/// `hitTestChildren(...) || hitTestSelf(position)`, where `hitTestSelf`
/// defaults to `false` (invisible wrappers are transparent to hit-testing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTestSelf {
    /// Absorb the hit: join the hit path AND block elements behind (earlier
    /// siblings / lower z). For elements that paint a surface (colored
    /// `Container`, `Text`, `Image`, …) or are explicit hit targets
    /// (`PointerInteract` / `MouseRegion` with the default `Opaque`
    /// behavior).
    Opaque,
    /// Join the hit path WITHOUT absorbing — the walk continues past this
    /// element, so things behind it can still be hit. `HitTestBehavior::
    /// Translucent` on the gesture widgets.
    Translucent,
    /// Contribute nothing by itself; hit-testing defers entirely to children
    /// (the default — Flutter's `hitTestSelf == false`).
    Defer,
}

pub trait ElementRender: 'static {
    fn type_name(&self) -> &'static str;

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    );

    /// The **bounds gate**: whether `position` (element-local) is inside
    /// this element's box at all. Children are never tested outside it.
    /// This is a containment check only — it does NOT make the element
    /// absorb hits (see [`Self::hit_test_self`]).
    fn hit_test_bounds(&self, position: Offset, layout: &ComputedLayout) -> bool {
        position.x >= 0.0
            && position.x < layout.size.width
            && position.y >= 0.0
            && position.y < layout.size.height
    }

    /// Whether this element absorbs / joins a hit **by itself** once a point
    /// is inside its bounds (see [`HitTestSelf`]). Children are tested
    /// first — a subtree hit always keeps this element on the hit path as
    /// an ancestor, regardless of what this returns.
    fn hit_test_self(&self, position: Offset, layout: &ComputedLayout) -> HitTestSelf {
        let _ = (position, layout);
        HitTestSelf::Defer
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
