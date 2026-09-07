use crate::core::layout::{ComputedLayout, Offset};

use crate::builtin_plugins::text::elements::text_shared::paint_helpers;
use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, HitTestSelf, PaintContext};

use super::element::TextElement;

impl ElementRender for TextElement {
    fn type_name(&self) -> &'static str {
        "tur_paragraph"
    }

    // Text paints a surface — it absorbs hits within its bounds (Flutter
    // `RenderParagraph`), so selection / gestures over text target the text.
    fn hit_test_self(&self, _position: Offset, _layout: &ComputedLayout) -> HitTestSelf {
        HitTestSelf::Opaque
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        _layout: &ComputedLayout,
        _children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        let Some(ref layout_data) = self.cached_layout else {
            return;
        };

        if paint_ctx.is_focused() && self.selection_anchor != self.selection_end {
            let (s, e) = if self.selection_anchor < self.selection_end {
                (self.selection_anchor, self.selection_end)
            } else {
                (self.selection_end, self.selection_anchor)
            };
            paint_helpers::paint_selection(canvas, layout_data, s, e);
        }

        canvas.fill_text_layout(Offset::ZERO, layout_data);
    }
}
