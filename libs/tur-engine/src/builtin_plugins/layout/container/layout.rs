use crate::core::layout::{Constraints, EdgeInsets, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::ContainerElement;

impl ElementLayout for ContainerElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let width = cx.read_val_opt(self.view.width.as_ref());
        let height = cx.read_val_opt(self.view.height.as_ref());
        let padding = cx.read_val_opt(self.view.padding.as_ref());
        let alignment = cx.read_val_opt(self.view.alignment.as_ref());

        // Resolve all reactive paint props here (layout holds the store +
        // Context); paint reads `self.painting` and never touches the store.
        self.painting = super::element::ContainerPainting {
            color: cx.read_val_opt(self.view.color.as_ref()),
            border_color: cx.read_val_opt(self.view.border_color.as_ref()),
            border_width: cx.read_val_opt(self.view.border_width.as_ref()),
            border_radius: cx.read_val_opt(self.view.border_radius.as_ref()),
            border_position: cx
                .read_val_opt(self.view.border_position.as_ref())
                .unwrap_or_default(),
            shadow_color: cx.read_val_opt(self.view.shadow_color.as_ref()),
            shadow_blur: cx.read_val_opt(self.view.shadow_blur.as_ref()),
        };

        let sized_constraints = Constraints {
            min_width: width.unwrap_or(constraints.min_width),
            max_width: width.unwrap_or(constraints.max_width),
            min_height: height.unwrap_or(constraints.min_height),
            max_height: height.unwrap_or(constraints.max_height),
        };

        let padding_ed = padding.map(EdgeInsets::all);
        let padding_constraints = match padding_ed {
            Some(p) => sized_constraints.deflate(p),
            None => sized_constraints,
        };

        let inner_constraints = if alignment.is_some() {
            Constraints::loose(Size::new(
                padding_constraints.max_width,
                padding_constraints.max_height,
            ))
        } else {
            padding_constraints
        };

        let child_size = if let Some(&child_id) = children.first() {
            cx.layout_child(child_id, &inner_constraints)
        } else {
            inner_constraints.constrain(Size::ZERO)
        };

        let inflated = match padding_ed {
            Some(p) => p.inflate_size(child_size),
            None => child_size,
        };

        let size = sized_constraints.constrain(inflated);

        // --- position (assign child offset) ---
        if let Some(&child_id) = children.first() {
            let padding = cx.read_val_opt(self.view.padding.as_ref()).unwrap_or(0.0);
            let alignment = cx.read_val_opt(self.view.alignment.as_ref());
            let offset = match alignment {
                Some(ref align) => {
                    let inner_size = Size::new(
                        (size.width - padding * 2.0).max(0.0),
                        (size.height - padding * 2.0).max(0.0),
                    );
                    let child_size = cx.child_computed_size(child_id);
                    let inner_offset = align.align_offset(inner_size, child_size);
                    Offset::new(padding + inner_offset.x, padding + inner_offset.y)
                }
                None => Offset::new(padding, padding),
            };
            cx.set_child_offset(child_id, offset);
        }

        size
    }
}
