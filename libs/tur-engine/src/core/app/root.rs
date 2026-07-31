//! Engine-owned root wrapper.
//!
//! `render(userView)` mounts the user's view inside a `RootElement`. The
//! wrapper is mandatory: the user's view may be a fragment (`Switch` / `Each`
//! / `Condition` / `Fragment`) which has no `perform_layout` of its own, so
//! the engine needs a layout-capable element at the root.
//!
//! `RootElement` is a minimal vertical stack: children are laid out
//! top-to-bottom with their natural sizes (loose constraints forwarded to
//! each child), horizontally centered within the wrapper's width. The
//! wrapper itself adopts the root constraints (tight: `min == max ==
//! viewport`), so it always fills the viewport. This mirrors the historical
//! behavior of wrapping the user's view in `FlexView { direction: Vertical,
//! main_axis_size: Max }`, but the implementation is a tiny dedicated type
//! in `core/` so the engine has zero coupling to the layout plugin.
//!
//! No flex-item (`Expanded`) handling: root children are never flex items.
//! No `main_axis_size::Min` mode: root always fills the viewport.

use boa_engine::Context;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::layout::{
    Constraints, ElementLayout, ElementSubscribe, LayoutContext, Offset, Size, SubscribeCx,
};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::core::layout::ComputedLayout;
use crate::core::view::{Lifecycle, View, ViewCx};

/// The wrapper view. Has exactly one child (the user's view); the wrapper
/// element adopts whatever children the user's view produces (a transparent
/// Fragment exposes multiple children directly under the wrapper).
pub struct RootView {
    pub(crate) child: std::rc::Rc<dyn View>,
}

impl View for RootView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(id, AnyElement::new(RootElement), boa);
        let _child_id = self.child.build(cx, boa, id.into());
        cx.link_child(parent, id.into());
        id.into()
    }
}

/// The wrapper element. Vertical-stack layout with cross-axis centering.
pub struct RootElement;

impl Lifecycle for RootElement {}

impl ElementLayout for RootElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        // Pass LOOSE constraints to each child (min=0, max=viewport) so a
        // child can size itself naturally rather than being stretched to the
        // root's tight constraints.
        let child_constraints = Constraints {
            min_width: 0.0,
            max_width: constraints.max_width,
            min_height: 0.0,
            max_height: constraints.max_height,
        };

        let mut y: f64 = 0.0;
        let mut max_child_width: f64 = 0.0;
        for &child_id in children {
            let size = cx.layout_child(child_id, &child_constraints);
            // Cross-axis centering (matches the old FlexView default of
            // `CrossAxisAlignment::Center`): horizontally center each child
            // within the wrapper's width.
            let x = ((constraints.max_width - size.width) / 2.0).max(0.0);
            cx.set_child_offset(child_id, Offset::new(x, y));
            y += size.height;
            max_child_width = max_child_width.max(size.width);
        }

        // Root fills the viewport (tight constraints).
        constraints.constrain(Size::new(max_child_width, y))
    }
}

impl ElementRender for RootElement {
    fn type_name(&self) -> &'static str { "tur_root" }
    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas);
        }
    }
}

impl ElementSubscribe for RootElement {
    fn subscribe(&self, _cx: &mut SubscribeCx) {}
}

impl ElementTrace for RootElement {
    fn trace_label(&self) -> String { String::new() }
    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> { Vec::new() }
}
