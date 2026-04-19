use tur_shared::{Constraints, EdgeInsets, Offset, Size};

use crate::{ChildLayout, ChildPaint, ComputedLayout, PaintContext, RenderNodeId, RenderObject};

#[derive(Debug)]
pub struct ContainerRenderObject {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub padding: Option<f64>,
    pub color: Option<String>,
}

impl ContainerRenderObject {
    pub fn new(
        width: Option<f64>,
        height: Option<f64>,
        padding: Option<f64>,
        color: Option<String>,
    ) -> Self {
        ContainerRenderObject {
            width,
            height,
            padding,
            color,
        }
    }
}

impl RenderObject for ContainerRenderObject {
    fn type_name(&self) -> &'static str {
        "tur_container"
    }

    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[RenderNodeId],
        child_layout: &mut dyn ChildLayout,
    ) -> Size {
        let sized_constraints = Constraints {
            min_width: self.width.unwrap_or(constraints.min_width),
            max_width: self.width.unwrap_or(constraints.max_width),
            min_height: self.height.unwrap_or(constraints.min_height),
            max_height: self.height.unwrap_or(constraints.max_height),
        };

        let padding = self.padding.map(EdgeInsets::all);
        let inner_constraints = match padding {
            Some(p) => sized_constraints.deflate(p),
            None => sized_constraints,
        };

        let child_size = if let Some(&child_id) = children.first() {
            child_layout.layout_child(child_id, &inner_constraints)
        } else {
            inner_constraints.constrain(Size::ZERO)
        };

        let inflated = match padding {
            Some(p) => p.inflate_size(child_size),
            None => child_size,
        };

        sized_constraints.constrain(inflated)
    }

    fn perform_layout_position(
        &mut self,
        _children: &[RenderNodeId],
        _child_layout: &mut dyn ChildLayout,
    ) {
    }

    fn paint(
        &self,
        ctx: &mut dyn PaintContext,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[RenderNodeId],
        child_paint: &mut dyn ChildPaint,
    ) {
        if let Some(ref color) = self.color {
            ctx.fill_rect(offset, layout.size, color);
        }
        for &child_id in children {
            child_paint.paint_child(child_id, ctx, offset);
        }
    }
}
