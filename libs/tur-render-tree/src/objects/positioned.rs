use tur_trait::{
    ChildLayout, ChildPaint, ComputedLayout, Constraints, Offset, PaintContext, RenderNodeId,
    RenderObject, Size,
};

#[derive(Debug)]
pub struct PositionedRenderObject {
    pub left: Option<f64>,
    pub top: Option<f64>,
    pub right: Option<f64>,
    pub bottom: Option<f64>,
}

impl PositionedRenderObject {
    pub fn new(
        left: Option<f64>,
        top: Option<f64>,
        right: Option<f64>,
        bottom: Option<f64>,
    ) -> Self {
        PositionedRenderObject {
            left,
            top,
            right,
            bottom,
        }
    }
}

impl RenderObject for PositionedRenderObject {
    fn type_name(&self) -> &'static str {
        "tur_positioned"
    }

    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[RenderNodeId],
        child_layout: &mut dyn ChildLayout,
    ) -> Size {
        let child_constraints = match (self.left, self.right, self.top, self.bottom) {
            (Some(_), Some(_), Some(_), Some(_)) => {
                let w =
                    (constraints.max_width - self.left.unwrap_or(0.0) - self.right.unwrap_or(0.0))
                        .max(0.0);
                let h =
                    (constraints.max_height - self.top.unwrap_or(0.0) - self.bottom.unwrap_or(0.0))
                        .max(0.0);
                Constraints::tight(Size::new(w, h))
            }
            _ => Constraints::loose(
                constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
            ),
        };

        if let Some(&child_id) = children.first() {
            child_layout.layout_child(child_id, &child_constraints)
        } else {
            child_constraints.constrain(Size::ZERO)
        }
    }

    fn perform_layout_position(
        &mut self,
        _children: &[RenderNodeId],
        child_layout: &mut dyn ChildLayout,
    ) {
        let offset_x = self.left.unwrap_or(0.0);
        let offset_y = self.top.unwrap_or(0.0);
        child_layout.set_child_offset_self(Offset::new(offset_x, offset_y));
    }

    fn paint(
        &self,
        _ctx: &mut dyn PaintContext,
        _offset: Offset,
        _layout: &ComputedLayout,
        children: &[RenderNodeId],
        child_paint: &mut dyn ChildPaint,
    ) {
        for &child_id in children {
            child_paint.paint_child(child_id, _ctx, _offset);
        }
    }
}
