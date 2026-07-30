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
    Size, SubscribeCx,
};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::core::view::{Lifecycle, Val, View, ViewCx};

use super::link::{extract_link_state, CompositedLinkState};

#[derive(Clone)]
pub struct FollowerView {
    pub(super) link: Option<Rc<CompositedLinkState>>,
    pub(super) target_anchor: Val<Alignment>,
    pub(super) follower_anchor: Val<Alignment>,
    /// `targetOffset` is a `{ x, y }` object (Flutter's `offset`), held as a
    /// `Val<JsValue>` because the object can only be field-read WITH a `Context`
    /// (`FromJs` is context-free by design). Resolved to an `Offset` during
    /// layout and cached on the element for the subsystem to read.
    pub(super) target_offset: Option<Val<JsValue>>,
    pub(super) show_when_unlinked: bool,
    pub(super) child: Option<Rc<dyn View>>,
}

impl View for FollowerView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(FollowerElement {
                view: self.clone(),
                // Defaults match the Val defaults (TopLeft/TopLeft/zero); the
                // first `perform_layout` overwrites them with the resolved
                // values before the subsystem reads them.
                resolved_target_anchor: Alignment::TopLeft,
                resolved_follower_anchor: Alignment::TopLeft,
                resolved_target_offset: Offset::ZERO,
            }),
            boa,
        );
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
    /// Resolved (reactive-decoded) anchor on the target — read by the
    /// subsystem via [`Self::desired_origin`]. Refreshed each `perform_layout`.
    pub(super) resolved_target_anchor: Alignment,
    pub(super) resolved_follower_anchor: Alignment,
    pub(super) resolved_target_offset: Offset,
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
        // Anchors/offset are resolved reactively in `perform_layout` and cached
        // here — the subsystem (which has no reactive store access) reads the
        // cache. The fixed-point flush loop guarantees a fresh value is laid
        // out before the subsystem reads it within the same frame.
        let target_anchor_pt = self
            .resolved_target_anchor
            .align_offset(target_size, Size::ZERO);
        let follower_anchor_pt = self
            .resolved_follower_anchor
            .align_offset(follower_size, Size::ZERO);
        let target_local = Point::new(
            target_anchor_pt.x + self.resolved_target_offset.x,
            target_anchor_pt.y + self.resolved_target_offset.y,
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
impl ElementSubscribe for FollowerElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        cx.subscribe_val(&self.view.target_anchor);
        cx.subscribe_val(&self.view.follower_anchor);
        if let Some(v) = self.view.target_offset.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

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
        // Resolve reactive props and cache for the subsystem. `targetOffset`
        // is a `{ x, y }` object held as `Val<JsValue>` — decode it with the
        // layout JS face (object field access needs a `Context`).
        self.resolved_target_anchor = cx
            .read_val(&self.view.target_anchor)
            .unwrap_or(Alignment::TopLeft);
        self.resolved_follower_anchor = cx
            .read_val(&self.view.follower_anchor)
            .unwrap_or(Alignment::TopLeft);
        let offset_js: Option<JsValue> = self
            .view
            .target_offset
            .as_ref()
            .and_then(|v| cx.read_val(v));
        self.resolved_target_offset = match &offset_js {
            Some(v) => decode_offset(v, cx.js.boa_mut()),
            None => Offset::ZERO,
        };

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
        let target_anchor = p
            .val::<Alignment>("targetAnchor")
            .unwrap_or(Val::Static(Alignment::TopLeft));
        let follower_anchor = p
            .val::<Alignment>("followerAnchor")
            .unwrap_or(Val::Static(Alignment::TopLeft));
        let show_when_unlinked = p
            .opt::<bool>("showWhenUnlinked")
            .unwrap_or(true);
        let child = p.child("child");
        // `targetOffset` is a static `{ x, y }` object or a `Val` of one; held
        // as a raw `Val<JsValue>` and field-decoded at layout time (see
        // `perform_layout` / `decode_offset`).
        let target_offset = p.val::<JsValue>("targetOffset");
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

/// Decode a `{ x, y }` JS object into an `Offset`. Requires a `Context`
/// (object field access), so this runs at layout time via the JS face rather
/// than in the context-free `FromJs` path.
fn decode_offset(v: &JsValue, ctx: &mut Context) -> Offset {
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
