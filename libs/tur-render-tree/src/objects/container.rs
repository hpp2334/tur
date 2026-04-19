use tur_shared::{ComputedLayout, Constraints, EdgeInsets, ElementKind, Offset, Size};

use crate::render_object::{ChildLayout, ChildPaint, PaintContext, RenderObject};
use crate::RenderNodeId;

#[derive(Debug)]
pub struct ContainerRenderObject {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub padding: Option<f64>,
    pub color: Option<String>,
}

impl ContainerRenderObject {
    pub fn from_props(props: &std::collections::HashMap<String, tur_element::PropValue>) -> Self {
        ContainerRenderObject {
            width: super::prop_f64(props, "width"),
            height: super::prop_f64(props, "height"),
            padding: super::prop_f64(props, "padding"),
            color: super::prop_str(props, "color").map(String::from),
        }
    }
}

impl RenderObject for ContainerRenderObject {
    fn kind(&self) -> ElementKind {
        ElementKind::Container
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
