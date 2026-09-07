use crate::core::layout::{ComputedLayout, HitTestBehavior, Offset};
use crate::core::shell::Cursor;

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, HitTestSelf, PaintContext};

use super::element::MouseRegionElement;

impl ElementRender for MouseRegionElement {
    fn type_name(&self) -> &'static str {
        "tur_mouse_region"
    }

    // Flutter `MouseRegion` defaults to `opaque: true` — the region absorbs
    // hits (hover + enter/exit work over empty content); `Translucent`
    // joins the hit path without blocking what's behind.
    fn hit_test_self(&self, _position: Offset, _layout: &ComputedLayout) -> HitTestSelf {
        match self.behavior {
            HitTestBehavior::Opaque => HitTestSelf::Opaque,
            HitTestBehavior::Translucent => HitTestSelf::Translucent,
        }
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        // Resolve the cursor during paint. Paint runs shallow→deep, so the
        // deepest MouseRegion under the pointer writes last (deepest wins).
        // An opaque region under the pointer claims `Default` first, dropping
        // any cursor already written by its ancestors (which painted
        // earlier) — mirroring `filter_opaque_path`'s ancestor exclusion.
        if paint_ctx.pointer_inside(&layout.size) {
            if self.is_region_opaque() {
                paint_ctx.set_cursor(Cursor::Default);
            }
            if let Some(cursor) = self.resolved_cursor() {
                paint_ctx.set_cursor(cursor);
            }
        }

        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas);
        }
    }
}
