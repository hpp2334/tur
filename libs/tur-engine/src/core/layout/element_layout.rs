use tur_shared::{Constraints, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::LayoutContext;

pub trait ElementLayout: Send + Sync + 'static {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size;

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext);
}
