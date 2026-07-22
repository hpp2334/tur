use crate::core::layout::{Constraints, Size};

use crate::core::element::ElementNodeId;
use crate::core::image_resource::ImageResourceId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::ImageElement;

impl ElementLayout for ImageElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let resource_id = cx
            .read_val_opt(self.view.resource_id.as_ref())
            .map(ImageResourceId::new);

        // Resolve paint props here (layout holds the store); paint reads
        // `self.painting` and never touches the store.
        self.painting = super::element::ImagePainting {
            resource_id: cx.read_val_opt(self.view.resource_id.as_ref()),
            fit: cx.read_val_opt(self.view.fit.as_ref()),
        };

        let width = cx.read_val_opt(self.view.width.as_ref());
        let height = cx.read_val_opt(self.view.height.as_ref());

        let natural = resource_id
            .and_then(|rid| cx.get_image_natural_size(rid))
            .unwrap_or(Size::ZERO);

        let w = width.unwrap_or_else(|| {
            if constraints.max_width.is_finite() {
                constraints.max_width
            } else {
                natural.width
            }
        });
        let h = height.unwrap_or_else(|| {
            if constraints.max_height.is_finite() {
                constraints.max_height
            } else {
                natural.height
            }
        });

        constraints.constrain(Size::new(w, h))
    }
}
