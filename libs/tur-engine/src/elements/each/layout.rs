use tur_shared::{Constraints, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::EachElement;

// `EachElement` is a transparent relay to a `FlexElement` layout: it forwards the incoming
// constraints to its mounted item children (laid out as a vertical flex via
// the `FlexElement` delegate held on the element), positions them, and paints
// nothing itself.

impl ElementLayout for EachElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        self.flex.perform_layout(constraints, children, cx)
    }
}
