use tur_shared::{Constraints, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::LayoutContext;

pub trait ElementLayout: 'static {
    /// Lay out this element: measure its children (via `cx.layout_child`),
    /// compute its own size, and assign each child's offset (via
    /// `cx.set_child_offset`). Children are fully laid out (their own
    /// `perform_layout` runs) before this element assigns their offsets.
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size;
}
