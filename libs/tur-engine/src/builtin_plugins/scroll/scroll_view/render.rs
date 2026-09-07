use crate::core::layout::{ComputedLayout, Geometry, Offset};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, HitTestSelf, PaintContext};

use super::element::ScrollViewElement;

impl ElementRender for ScrollViewElement {
    fn type_name(&self) -> &'static str {
        "tur_scroll_view"
    }

    // The scrollable absorbs hits across its viewport (Flutter's `Scrollable`
    // wraps its viewport in an opaque `Listener`, so wheel/drag work over
    // empty content areas too).
    fn hit_test_self(&self, _position: Offset, _layout: &ComputedLayout) -> HitTestSelf {
        HitTestSelf::Opaque
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        if let Some(ref brush) = self.painting.color {
            canvas.fill_geometry(Offset::ZERO, &Geometry::Rect(layout.size), brush);
        }

        canvas.push_clip(Offset::ZERO, layout.size);
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas);
        }
        canvas.pop_clip();
    }
}
