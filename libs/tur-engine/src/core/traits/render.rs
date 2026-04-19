use tur_shared::{ComputedLayout, Offset};

use crate::core::render::{ChildPaint, PaintContext};
use crate::core::traits::ElementNodeId;

pub trait ElementRender: Send + Sync + 'static {
    fn type_name(&self) -> &'static str;

    fn paint(
        &self,
        ctx: &mut dyn PaintContext,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        child_paint: &mut dyn ChildPaint,
    );

    fn hit_test(&self, position: Offset, layout: &ComputedLayout) -> bool {
        position.x >= 0.0
            && position.x < layout.size.width
            && position.y >= 0.0
            && position.y < layout.size.height
    }
}
