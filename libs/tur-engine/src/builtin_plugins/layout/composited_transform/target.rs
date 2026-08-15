//! `CompositedTransformTarget` — a transparent passthrough that records its
//! node id on the shared [`LayerLink`], anchoring a follower to this spot in
//! the tree.

use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsResult, JsValue};

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::js_runtime::helpers::{Ptr, extract_js_ctx, require_props_object, wrap_view};
use crate::core::layout::{
    Constraints, ElementLayout, ElementSubscribe, LayoutContext, Offset, Size,
};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::core::view::{Lifecycle, View, ViewCx};

use super::link::{CompositedLinkState, extract_link_state};

#[derive(Clone, Default)]
pub struct TargetView {
    pub(super) link: Option<Rc<CompositedLinkState>>,
    pub(super) child: Option<Rc<dyn View>>,
}

impl View for TargetView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = cx.alloc_node().as_element_id();
        cx.insert_node(id, AnyElement::new(TargetElement), boa);
        if let Some(state) = &self.link {
            state.target_node.set(Some(id));
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, boa, id.into());
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

impl TargetView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let link = extract_link_state(props, ctx)?;
        Some(TargetView {
            link: Some(link),
            child: crate::core::js_runtime::JsProps::new(props, ctx).child("child"),
        })
    }
}

pub(super) fn tur_target(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_js_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = TargetView::from_js(&props, context).ok_or_else(|| {
        boa_engine::JsError::from(
            boa_engine::JsNativeError::typ()
                .with_message("CompositedTransformTarget requires a `link` (createLayerLink())"),
        )
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}

#[allow(dead_code)]
const _ENSURE_PTR: Ptr = tur_target as Ptr;
