use tur_shared::{Constraints, Size};

use crate::core::render::ChildLayout;
use crate::core::traits::ElementNodeId;

pub trait ElementLayout: Send + Sync + 'static {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        child_layout: &mut dyn ChildLayout,
    ) -> Size;

    fn perform_layout_position(
        &mut self,
        children: &[ElementNodeId],
        child_layout: &mut dyn ChildLayout,
    );
}
