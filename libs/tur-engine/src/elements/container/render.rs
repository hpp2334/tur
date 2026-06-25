use tur_shared::{
    BorderPosition, ComputedLayout, Constraints, EdgeInsets, Geometry, Offset, Size,
};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

use super::element::ContainerElement;

impl ElementLayout for ContainerElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let width = cx.read_val_opt(self.component.width.as_ref());
        let height = cx.read_val_opt(self.component.height.as_ref());
        let padding = cx.read_val_opt(self.component.padding.as_ref());
        let alignment = cx.read_val_opt(self.component.alignment.as_ref());

        // Resolve all reactive paint props here (layout holds the store +
        // Context); paint reads `self.painting` and never touches the store.
        self.painting = super::element::ContainerPainting {
            color: cx.read_val_opt(self.component.color.as_ref()),
            border_color: cx.read_val_opt(self.component.border_color.as_ref()),
            border_width: cx.read_val_opt(self.component.border_width.as_ref()),
            border_radius: cx.read_val_opt(self.component.border_radius.as_ref()),
            border_position: cx
                .read_val_opt(self.component.border_position.as_ref())
                .unwrap_or_default(),
            shadow_color: cx.read_val_opt(self.component.shadow_color.as_ref()),
            shadow_blur: cx.read_val_opt(self.component.shadow_blur.as_ref()),
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

        sized_constraints.constrain(inflated)
    }

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        if let Some(&child_id) = children.first() {
            let padding = cx.read_val_opt(self.component.padding.as_ref()).unwrap_or(0.0);
            let alignment = cx.read_val_opt(self.component.alignment.as_ref());
            let container_size = cx.self_computed_size();
            let offset = match alignment {
                Some(ref align) => {
                    let inner_size = Size::new(
                        (container_size.width - padding * 2.0).max(0.0),
                        (container_size.height - padding * 2.0).max(0.0),
                    );
                    let child_size = cx.child_computed_size(child_id);
                    let inner_offset = align.align_offset(inner_size, child_size);
                    Offset::new(padding + inner_offset.x, padding + inner_offset.y)
                }
                None => Offset::new(padding, padding),
            };
            cx.set_child_offset(child_id, offset);
        }
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
        let p = &self.painting;
        let shadow_blur = p.shadow_blur;
        let shadow_color = p.shadow_color.as_ref();
        let color = p.color.as_ref();
        let border_color = p.border_color.as_ref();
        let border_width = p.border_width;
        let border_radius = p.border_radius;
        let border_position = p.border_position;

        if let (Some(sc), Some(sb)) = (shadow_color, shadow_blur) {
            if sb > 0.0 {
                let shadow_offset = self.component.shadow_offset.unwrap_or((0.0, 0.0));
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
}
