use crate::core::layout::ComputedLayout;

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, HitTestSelf, PaintContext};

use super::element::PointerInteractElement;

impl ElementRender for PointerInteractElement {
    fn type_name(&self) -> &'static str {
        "tur_pointer_interact"
    }

    // The explicit hit-target widget: `Opaque` (the default) absorbs — a
    // bare `PointerInteract` with no visible child stays clickable.
    // `Translucent` joins the hit path without blocking what's behind.
    fn hit_test_self(
        &self,
        _position: crate::core::layout::Offset,
        _layout: &ComputedLayout,
    ) -> HitTestSelf {
        match self.behavior() {
            crate::core::layout::HitTestBehavior::Opaque => HitTestSelf::Opaque,
            crate::core::layout::HitTestBehavior::Translucent => HitTestSelf::Translucent,
        }
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas);
        }
    }
}
