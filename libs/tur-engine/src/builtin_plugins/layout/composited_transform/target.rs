//! `CompositedTransformTarget` — a transparent passthrough that records its
//! node id on the shared [`LayerLink`], anchoring a follower to this spot in
//! the tree.

use std::rc::Rc;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::layout::{
    Constraints, ElementLayout, ElementSubscribe, LayoutContext, Offset, Size,
};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::core::view::{Lifecycle, View, ViewCx};

use super::link::CompositedLinkState;

#[derive(Clone, Default)]
pub struct TargetView {
    pub(super) link: Option<Rc<CompositedLinkState>>,
    pub(super) child: Option<Rc<dyn View>>,
}

impl TargetView {
    /// Rut-rail constructor (`core::rut_runtime`): link + child.
    pub fn new_rut(link: Option<Rc<CompositedLinkState>>, child: Option<Rc<dyn View>>) -> Self {
        Self { link, child }
    }
}

impl View for TargetView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(id, AnyElement::new(TargetElement));
        if let Some(state) = &self.link {
            state.target_node.set(Some(id));
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

/// Stateless passthrough — the link binding happens in `TargetView::build`.
pub struct TargetElement;

impl Lifecycle for TargetElement {}
impl ElementSubscribe for TargetElement {}

impl ElementTrace for TargetElement {
    fn trace_label(&self) -> String {
        String::new()
    }
}

impl ElementLayout for TargetElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let size = if let Some(child_id) = children.first() {
            cx.layout_child(*child_id, constraints)
        } else {
            constraints.constrain(Size::ZERO)
        };
        if let Some(child_id) = children.first() {
            cx.set_child_offset(*child_id, Offset::ZERO);
        }
        size
    }
}

impl ElementRender for TargetElement {
    fn type_name(&self) -> &'static str {
        "tur_composited_transform_target"
    }

    fn paint(
        &self,
        _canvas: &mut dyn Canvas,
        _layout: &crate::core::layout::ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        for &child_id in children {
            paint_ctx.paint_child(child_id, _canvas);
        }
    }
}
