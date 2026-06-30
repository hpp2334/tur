use tur_shared::{Constraints, EdgeInsets, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use super::element::AnimatedContainerElement;

// Lerp helpers (mirror the closures used in the Effect phase).
fn lerp_f64(a: &f64, b: &f64, t: f64) -> f64 {
    a + (b - a) * t
}

impl ElementLayout for AnimatedContainerElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let t = self.host.eased_t();

        // Resolve each animatable prop at the eased `t`. On first layout
        // (before any tick) the host reports `t = 1.0`, so the seeds
        // (`begin == end == target`) paint the target immediately.
        let width = self.p_width.evaluate(t, lerp_f64);
        let height = self.p_height.evaluate(t, lerp_f64);
        let padding = self.p_padding.evaluate(t, lerp_f64);

        // Non-animatable reactive props still resolve from the store each
        // layout, exactly like ContainerElement.
        let alignment = cx.read_val_opt(self.view.alignment.as_ref());

        // Resolve paint-only animatable props into `painting`.
        self.painting = crate::elements::container::ContainerPainting {
            color: self
                .p_color
                .evaluate(t, super::element::lerp_brush),
            border_color: self
                .p_border_color
                .evaluate(t, |a, b, tt| tur_shared::Color::lerp(*a, *b, tt)),
            border_width: self.p_border_width.evaluate(t, lerp_f64),
            border_radius: self.p_border_radius.evaluate(t, lerp_f64),
            border_position: cx
                .read_val_opt(self.view.border_position.as_ref())
                .unwrap_or_default(),
            shadow_color: self
                .p_shadow_color
                .evaluate(t, |a, b, tt| tur_shared::Color::lerp(*a, *b, tt)),
            shadow_blur: self.p_shadow_blur.evaluate(t, lerp_f64),
        };

        // --- sizing: identical to ContainerElement, but using the animated
        //     width/height/padding values so the box grows/shrinks smoothly.
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
            let padding = padding.unwrap_or(0.0);
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
