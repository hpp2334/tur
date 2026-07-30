//! `CompositedTransformFollower` — renders at a target's anchor, tracked
//! continuously by [`super::subsystem::CompositedTransformSubsystem`].
//!
//! The follower's `computed_layout.offset` is written by the subsystem each
//! flush (a pure translation), so paint + hit-testing work through the normal
//! offset accumulation. Place the follower in a root overlay slot (the Flutter
//! `Overlay` pattern) so it isn't clipped and paints on top.

use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{js_string, Context, JsResult, JsValue};
use vello_common::kurbo::Point;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::js_runtime::helpers::{extract_ctx, require_props_object, wrap_view, Ptr};
use crate::core::js_runtime::JsProps;
use crate::core::layout::{
    Alignment, ComputedLayout, Constraints, ElementLayout, ElementSubscribe, LayoutContext, Offset,
    Size,
};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::core::view::{Lifecycle, View, ViewCx};

use super::link::{extract_link_state, CompositedLinkState};

#[derive(Clone)]
pub struct FollowerView {
    pub(super) link: Option<Rc<CompositedLinkState>>,
    pub(super) target_anchor: Alignment,
    pub(super) follower_anchor: Alignment,
    pub(super) target_offset: Offset,
    pub(super) show_when_unlinked: bool,
    pub(super) child: Option<Rc<dyn View>>,
}

impl View for FollowerView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(id, AnyElement::new(FollowerElement { view: self.clone() }), boa);
        if let Some(state) = &self.link {
            state.follower_node.set(Some(id));
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, boa, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

pub struct FollowerElement {
    pub(super) view: FollowerView,
}

impl FollowerElement {
    /// The desired absolute (canvas-space) origin for the follower's top-left,
    /// computed from the target's world affine + size and this follower's
    /// anchors + `targetOffset`.
    ///
    /// `targetOffset` is expressed in the target's local coordinate space
    /// (matching Flutter): the point on the target that the follower's anchor
    /// aligns to is `targetAnchor + targetOffset`, mapped through the target's
    /// world transform. The follower is translated (not rotated) so its
    /// `followerAnchor` lands on that point.
    pub(crate) fn desired_origin(
        &self,
        target_world: vello_common::kurbo::Affine,
        target_size: Size,
        follower_size: Size,
    ) -> Offset {
        let target_anchor_pt = self.view.target_anchor.align_offset(target_size, Size::ZERO);
        let follower_anchor_pt = self
            .view
            .follower_anchor
            .align_offset(follower_size, Size::ZERO);
        let target_local = Point::new(
            target_anchor_pt.x + self.view.target_offset.x,
            target_anchor_pt.y + self.view.target_offset.y,
        );
        let global = target_world * target_local;
        Offset::new(global.x - follower_anchor_pt.x, global.y - follower_anchor_pt.y)
    }

    pub(crate) fn linked(&self) -> bool {
        self.view
            .link
            .as_ref()
            .map(|s| s.linked.get())
            .unwrap_or(false)
    }
}

impl Lifecycle for FollowerElement {}
impl ElementSubscribe for FollowerElement {}

impl ElementTrace for FollowerElement {
    fn trace_label(&self) -> String {
        String::new()
    }
}

impl ElementLayout for FollowerElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        // The follower's own offset is assigned by the subsystem each flush
        // (it tracks the target); here we only size + place the child.
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

impl ElementRender for FollowerElement {
    fn type_name(&self) -> &'static str {
        "tur_composited_transform_follower"
    }

    fn paint(
        &self,
        _canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        if !self.view.show_when_unlinked && !self.linked() {
            return;
        }
        for &child_id in children {
            paint_ctx.paint_child(child_id, _canvas, offset);
        }
    }
}

impl FollowerView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let link = extract_link_state(props, ctx)?;
        let mut p = JsProps::new(props, ctx);
        let target_anchor = p.opt::<Alignment>("targetAnchor").unwrap_or(Alignment::TopLeft);
        let follower_anchor = p
            .opt::<Alignment>("followerAnchor")
            .unwrap_or(Alignment::TopLeft);
        let show_when_unlinked = p
            .opt::<bool>("showWhenUnlinked")
            .unwrap_or(true);
        let child = p.child("child");
        // `targetOffset` is a static `{x, y}` object (Flutter's `offset` is a
        // plain Offset). Decoded here with the context (object field access).
        let target_offset = read_offset(props, "targetOffset", p.ctx());
        Some(FollowerView {
            link: Some(link),
            target_anchor,
            follower_anchor,
            target_offset,
            show_when_unlinked,
            child,
        })
    }
}

fn read_offset(props: &JsObject, key: &str, ctx: &mut Context) -> Offset {
    let Ok(v) = props.get(js_string!(key), ctx) else {
        return Offset::ZERO;
    };
    let Some(obj) = v.as_object() else {
        return Offset::ZERO;
    };
    let x = obj
        .get(js_string!("x"), ctx)
        .ok()
        .and_then(|n| n.as_number())
        .unwrap_or(0.0);
    let y = obj
        .get(js_string!("y"), ctx)
        .ok()
        .and_then(|n| n.as_number())
        .unwrap_or(0.0);
    Offset::new(x, y)
}

pub(super) fn tur_follower(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let _ = extract_ctx(args)?;
    let props = require_props_object(args, 1, context)?;
    let spec = FollowerView::from_js(&props, context).ok_or_else(|| {
        boa_engine::JsError::from(
            boa_engine::JsNativeError::typ()
                .with_message("CompositedTransformFollower requires a `link` (createLayerLink())"),
        )
    })?;
    Ok(wrap_view(Rc::new(spec), context))
}

#[allow(dead_code)]
const _ENSURE_PTR: Ptr = tur_follower as Ptr;
