use crate::core::layout::{BoxFit, ComputedLayout};

use crate::core::element::ElementNodeId;
use crate::core::image_resource::ImageResourceId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::ImageElement;

impl ElementRender for ImageElement {
    fn type_name(&self) -> &'static str {
        "tur_image"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        let rid = match self.painting.resource_id.map(ImageResourceId::new) {
            Some(id) => id,
            None => {
                for &child_id in children {
                    paint_ctx.paint_child(child_id, canvas);
                }
                return;
            }
        };

        let img_res = match paint_ctx.get_image_resource(rid) {
            Some(r) => r,
            None => {
                for &child_id in children {
                    paint_ctx.paint_child(child_id, canvas);
                }
                return;
            }
        };

        let natural_w = img_res.natural_size.width;
        let natural_h = img_res.natural_size.height;
        if natural_w > 0.0 && natural_h > 0.0 {
            let fit = self.painting.fit.unwrap_or_default();

            let layout_w = layout.size.width;
            let layout_h = layout.size.height;

            let (draw_w, draw_h, offset_x, offset_y) =
                compute_box_fit(fit, natural_w, natural_h, layout_w, layout_h);

            let scale_x = draw_w / natural_w;
            let scale_y = draw_h / natural_h;

            // Local BoxFit transform: the canvas transform already positions
            // the element at its absolute origin, so only the BoxFit
            // letterbox/pillarbox offset + scale remain.
            let transform = vello_common::kurbo::Affine::translate((offset_x, offset_y))
                * vello_common::kurbo::Affine::scale_non_uniform(scale_x, scale_y);

            canvas.draw_image(rid, img_res.natural_size, transform);
        }

        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas);
        }
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
