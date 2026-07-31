use crate::core::layout::{BorderPosition, ComputedLayout, Geometry, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::ContainerElement;

impl ElementRender for ContainerElement {
    fn type_name(&self) -> &'static str {
        "tur_container"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        let shadow_blur = self.painting.shadow_blur;
        let shadow_color = self.painting.shadow_color.as_ref();
        let color = self.painting.color.as_ref();
        let border_color = self.painting.border_color.as_ref();
        let border_width = self.painting.border_width;
        let border_radius = self.painting.border_radius;
        let border_position = self.painting.border_position;

        if let (Some(sc), Some(sb)) = (shadow_color, shadow_blur)
            && sb > 0.0 {
                let radius = border_radius.unwrap_or(0.0);
                canvas.draw_shadow(
                    Offset::ZERO,
                    layout.size,
                    sc,
                    radius,
                    sb,
                    self.view.shadow_offset.unwrap_or((0.0, 0.0)),
                );
            }

        if let Some(brush) = color {
            let geometry = match border_radius {
                Some(r) if r > 0.0 => Geometry::RoundedRect {
                    size: layout.size,
                    radius: r,
                },
                _ => Geometry::Rect(layout.size),
            };
            canvas.fill_geometry(Offset::ZERO, &geometry, brush);
        }

        if let (Some(bc), Some(bw)) = (border_color, border_width)
            && bw > 0.0 {
                let half = bw / 2.0;
                let s = layout.size;
                let (ox, oy, size) = match border_position {
                    BorderPosition::Inside => (
                        half,
                        half,
                        Size::new((s.width - bw).max(0.0), (s.height - bw).max(0.0)),
                    ),
                    BorderPosition::Outside => (
                        -half,
                        -half,
                        Size::new(s.width + bw, s.height + bw),
                    ),
                    BorderPosition::Center => (0.0, 0.0, s),
                };
                // Local inset/outset — the canvas transform already positions
                // the box at its absolute origin.
                let border_offset = Offset::new(ox, oy);
                let geometry = match border_radius {
                    Some(r) if r > 0.0 => Geometry::RoundedRect { size, radius: r },
                    _ => Geometry::Rect(size),
                };
                canvas.stroke_geometry(border_offset, &geometry, bc, bw);
            }

        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas);
        }
    }
}
