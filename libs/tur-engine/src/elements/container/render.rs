use tur_shared::{BorderPosition, ComputedLayout, Geometry, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::{ContainerElement, ContainerPainting};

/// Shared paint routine for a container-shaped element (background fill,
/// shadow, border, then children). Used by both [`ContainerElement`] and the
/// implicit-animation family (`AnimatedContainerElement`) so they paint
/// identically.
///
/// `shadow_offset` is the parsed `[x, y]` from the view (non-reactive); the
/// rest comes from the resolved `painting` struct filled during layout.
pub(crate) fn paint_container_body(
    canvas: &mut dyn Canvas,
    offset: Offset,
    layout: &ComputedLayout,
    children: &[ElementNodeId],
    paint_ctx: &PaintContext,
    painting: &ContainerPainting,
    shadow_offset: (f64, f64),
) {
    let shadow_blur = painting.shadow_blur;
    let shadow_color = painting.shadow_color.as_ref();
    let color = painting.color.as_ref();
    let border_color = painting.border_color.as_ref();
    let border_width = painting.border_width;
    let border_radius = painting.border_radius;
    let border_position = painting.border_position;

    if let (Some(sc), Some(sb)) = (shadow_color, shadow_blur) {
        if sb > 0.0 {
            let radius = border_radius.unwrap_or(0.0);
            canvas.draw_shadow(offset, layout.size, sc, radius, sb, shadow_offset);
        }
    }

    if let Some(brush) = color {
        let geometry = match border_radius {
            Some(r) if r > 0.0 => Geometry::RoundedRect {
                size: layout.size,
                radius: r,
            },
            _ => Geometry::Rect(layout.size),
        };
        canvas.fill_geometry(offset, &geometry, brush);
    }

    if let (Some(bc), Some(bw)) = (border_color, border_width) {
        if bw > 0.0 {
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
            let border_offset = Offset::new(offset.x + ox, offset.y + oy);
            let geometry = match border_radius {
                Some(r) if r > 0.0 => Geometry::RoundedRect { size, radius: r },
                _ => Geometry::Rect(size),
            };
            canvas.stroke_geometry(border_offset, &geometry, bc, bw);
        }
    }

    for &child_id in children {
        paint_ctx.paint_child(child_id, canvas, offset);
    }
}

impl ElementRender for ContainerElement {
    fn type_name(&self) -> &'static str {
        "tur_container"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        paint_container_body(
            canvas,
            offset,
            layout,
            children,
            paint_ctx,
            &self.painting,
            self.view.shadow_offset.unwrap_or((0.0, 0.0)),
        );
    }
}
