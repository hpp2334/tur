use tur_shared::{ComputedLayout, Offset};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::AnimatedContainerElement;

impl ElementRender for AnimatedContainerElement {
    fn type_name(&self) -> &'static str {
        "tur_animated_container"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        crate::elements::container::paint_container_body(
            canvas,
            offset,
            layout,
            children,
            paint_ctx,
            &self.painting,
            self.view.shadow_offset.unwrap_or((0.0, 0.0)),
        );
    }
}
