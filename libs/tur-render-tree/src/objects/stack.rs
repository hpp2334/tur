use tur_trait::{
    ChildLayout, ChildPaint, ComputedLayout, Constraints, Offset, PaintContext, RenderNodeId,
    RenderObject, Size, StackFit,
};

#[derive(Debug)]
pub struct StackRenderObject {
    pub fit: StackFit,
}

impl StackRenderObject {
    pub fn new(fit: StackFit) -> Self {
        StackRenderObject { fit }
    }
}

impl RenderObject for StackRenderObject {
    fn type_name(&self) -> &'static str {
        "tur_stack"
    }

    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[RenderNodeId],
        child_layout: &mut dyn ChildLayout,
    ) -> Size {
        let mut max_size = Size::ZERO;

        for &child_id in children {
            let child_constraints = match self.fit {
                StackFit::Loose => Constraints::loose(
                    constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
                ),
                StackFit::Expand => Constraints::tight(
                    constraints.constrain(Size::new(constraints.max_width, constraints.max_height)),
                ),
                StackFit::Passthrough => *constraints,
            };
            let size = child_layout.layout_child(child_id, &child_constraints);
            max_size = Size::new(
                max_size.width.max(size.width),
                max_size.height.max(size.height),
            );
        }

        constraints.constrain(max_size)
    }

    fn perform_layout_position(
        &mut self,
        children: &[RenderNodeId],
        child_layout: &mut dyn ChildLayout,
    ) {
        for &child_id in children {
            let is_positioned = child_layout.get_child_type_name(child_id) == "tur_positioned";

            if !is_positioned {
                child_layout.set_child_offset(child_id, Offset::ZERO);
            }
        }
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
