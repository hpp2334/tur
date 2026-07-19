use tur_engine::core::layout::{Constraints, Size};

use tur_engine::core::element::ElementNodeId;
use tur_engine::core::layout::{ElementLayout, LayoutContext};

use super::element::{ScrollbarElement, DEFAULT_THICKNESS};

impl ElementLayout for ScrollbarElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        // Thickness is the scrollbar's own width; height fills whatever the
        // parent grants (the scroll viewport's cross axis).
        let thickness = cx
            .read_val_opt(self.view.thickness.as_ref())
            .unwrap_or(DEFAULT_THICKNESS);

        // Resolve paint props here (layout holds the store); paint reads
        // `self.painting` and never touches the store.
        self.painting = super::element::ScrollbarPainting {
            track_color: cx.read_val_opt(self.view.track_color.as_ref()),
            color: cx.read_val_opt(self.view.color.as_ref()),
            thumb_radius: cx.read_val_opt(self.view.thumb_radius.as_ref()),
        };

        let h = if constraints.max_height.is_finite() && constraints.max_height > 0.0 {
            constraints.max_height
        } else {
            0.0
        };
        let size = constraints.constrain(Size::new(thickness, h));
        self.cached_track = size;
        size
    }
}
