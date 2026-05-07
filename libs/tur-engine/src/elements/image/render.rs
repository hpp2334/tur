use tur_shared::{BoxFit, ComputedLayout, Constraints, Offset, Size};

use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::core::element::ElementNodeId;

use super::element::ImageElement;

impl ElementLayout for ImageElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let natural = self
            .resource_id
            .and_then(|rid| cx.get_image_natural_size(rid))
            .unwrap_or(Size::ZERO);

        let w = self.width.unwrap_or_else(|| {
            if constraints.max_width.is_finite() {
                constraints.max_width
            } else {
                natural.width
            }
        });
        let h = self.height.unwrap_or_else(|| {
            if constraints.max_height.is_finite() {
                constraints.max_height
            } else {
                natural.height
            }
        });

        constraints.constrain(Size::new(w, h))
    }

    fn perform_layout_position(&mut self, _children: &[ElementNodeId], _cx: &mut LayoutContext) {}
}

impl ElementRender for ImageElement {
    fn type_name(&self) -> &'static str {
        "tur_image"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        layout: &ComputedLayout,
        _children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        let rid = match self.resource_id {
            Some(id) => id,
            None => return,
        };

        let img_res = match paint_ctx.get_image_resource(rid) {
            Some(r) => r,
            None => return,
        };

        let natural_w = img_res.natural_size.width;
        let natural_h = img_res.natural_size.height;
        if natural_w <= 0.0 || natural_h <= 0.0 {
            return;
        }

        let layout_w = layout.size.width;
        let layout_h = layout.size.height;

        let (draw_w, draw_h, offset_x, offset_y) =
            compute_box_fit(self.fit, natural_w, natural_h, layout_w, layout_h);

        let scale_x = draw_w / natural_w;
        let scale_y = draw_h / natural_h;

        let transform = vello::kurbo::Affine::translate((
            offset.x + offset_x,
            offset.y + offset_y,
        )) * vello::kurbo::Affine::scale_non_uniform(scale_x, scale_y);

        canvas.draw_image(&img_res.peniko_image, transform);
    }
}

fn compute_box_fit(
    fit: BoxFit,
    natural_w: f64,
    natural_h: f64,
    layout_w: f64,
    layout_h: f64,
) -> (f64, f64, f64, f64) {
    match fit {
        BoxFit::Fill => (layout_w, layout_h, 0.0, 0.0),
        BoxFit::None => (natural_w, natural_h, 0.0, 0.0),
        BoxFit::Contain => {
            let scale = (layout_w / natural_w).min(layout_h / natural_h);
            let w = natural_w * scale;
            let h = natural_h * scale;
            let ox = (layout_w - w) / 2.0;
            let oy = (layout_h - h) / 2.0;
            (w, h, ox, oy)
        }
        BoxFit::Cover => {
            let scale = (layout_w / natural_w).max(layout_h / natural_h);
            let w = natural_w * scale;
            let h = natural_h * scale;
            let ox = (layout_w - w) / 2.0;
            let oy = (layout_h - h) / 2.0;
            (w, h, ox, oy)
        }
        BoxFit::FitWidth => {
            let scale = layout_w / natural_w;
            let h = natural_h * scale;
            let oy = (layout_h - h) / 2.0;
            (layout_w, h, 0.0, oy)
        }
        BoxFit::FitHeight => {
            let scale = layout_h / natural_h;
            let w = natural_w * scale;
            let ox = (layout_w - w) / 2.0;
            (w, layout_h, ox, 0.0)
        }
    }
}
