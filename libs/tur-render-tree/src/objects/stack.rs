use tur_shared::{ComputedLayout, Constraints, ElementKind, Offset, Size, StackFit};

use crate::render_object::{ChildLayout, ChildPaint, PaintContext, RenderObject};
use crate::RenderNodeId;

#[derive(Debug)]
pub struct StackRenderObject {
    pub fit: StackFit,
}

impl StackRenderObject {
    pub fn from_props(props: &std::collections::HashMap<String, tur_element::PropValue>) -> Self {
        let fit = super::prop_str(props, "fit")
            .and_then(|s| match s {
                "loose" => Some(StackFit::Loose),
                "expand" => Some(StackFit::Expand),
                "passthrough" => Some(StackFit::Passthrough),
                _ => None,
            })
            .or_else(|| {
                super::prop_f64(props, "fit").and_then(|n| match n as i32 {
                    0 => Some(StackFit::Loose),
                    1 => Some(StackFit::Expand),
                    2 => Some(StackFit::Passthrough),
                    _ => None,
                })
            })
            .unwrap_or(StackFit::Loose);

        StackRenderObject { fit }
    }
}

impl RenderObject for StackRenderObject {
    fn kind(&self) -> ElementKind {
        ElementKind::Stack
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
            let is_positioned = child_layout.get_child_kind(child_id) == ElementKind::Positioned;

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
