use tur_engine::core::layout::{ComputedLayout, Offset};

use tur_engine::core::element::ElementNodeId;
use tur_engine::core::render::{Canvas, ElementRender, PaintContext};
use crate::elements::text_shared::paint_helpers;

use super::element::TextElement;

impl ElementRender for TextElement {
    fn type_name(&self) -> &'static str {
        "tur_paragraph"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
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
            paint_helpers::paint_selection(canvas, offset, layout_data, s, e);
        }

        canvas.fill_text_layout(offset, layout_data);
    }
}
