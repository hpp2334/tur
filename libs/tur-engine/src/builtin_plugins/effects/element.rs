use std::rc::Rc;

use boa_engine::Context;
use boa_engine::object::JsObject;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::js_runtime::JsProps;
use crate::core::layout::{Alignment, ElementSubscribe, SubscribeCx};
use crate::core::view::{Lifecycle, Val, View, ViewCx};

// ---------------------------------------------------------------------------
// OpacityView — applies an alpha multiplier to its child subtree.
//
// `value` is the opacity in [0.0, 1.0] and is reactive.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct OpacityView {
    pub(crate) value: Option<Val<f32>>,
    pub(crate) query_key: Option<Vec<String>>,
    pub(crate) child: Option<Rc<dyn View>>,
}

impl View for OpacityView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = cx.alloc_node().as_element_id();
        cx.insert_node(
            id,
            AnyElement::new(OpacityElement {
                view: self.clone(),
                painting: OpacityPainting::default(),
            }),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, boa, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

pub struct OpacityElement {
    pub(crate) view: OpacityView,
    pub(crate) painting: OpacityPainting,
}

/// Resolved paint prop (filled during layout). Paint reads it directly.
#[derive(Clone)]
pub struct OpacityPainting {
    pub(crate) value: f32,
}
impl Default for OpacityPainting {
    fn default() -> Self {
        Self { value: 1.0 }
    }
}

impl Lifecycle for OpacityElement {}

impl ElementSubscribe for OpacityElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        if let Some(v) = self.view.value.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for OpacityElement {
    fn trace_label(&self) -> String {
        if let Some(Val::Static(v)) = &self.view.value {
            format!("opacity={v}")
        } else {
            String::new()
        }
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

impl OpacityView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        OpacityView {
            value: p.val::<f32>("value"),
            query_key: p.query_key("queryKey"),
            child: p.child("child"),
        }
    }
}

// ---------------------------------------------------------------------------
// TransformView — applies a 2D affine transform to its child subtree.
//
// Supported props: `scale` (uniform), `scaleX`, `scaleY`, `rotate` (radians),
// `translateX`, `translateY`, and `alignment` (the pivot for rotate/scale,
// defaulting to `Alignment.Center` — matches Flutter's `Transform`). All
// reactive.
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct TransformView {
    pub(crate) scale: Option<Val<f64>>,
    pub(crate) scale_x: Option<Val<f64>>,
    pub(crate) scale_y: Option<Val<f64>>,
    pub(crate) rotate: Option<Val<f64>>,
    pub(crate) translate_x: Option<Val<f64>>,
    pub(crate) translate_y: Option<Val<f64>>,
    pub(crate) alignment: Option<Val<Alignment>>,
    pub(crate) query_key: Option<Vec<String>>,
    pub(crate) child: Option<Rc<dyn View>>,
}

impl View for TransformView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = cx.alloc_node().as_element_id();
        cx.insert_node(
            id,
            AnyElement::new(TransformElement {
                view: self.clone(),
                painting: TransformPainting::default(),
            }),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, boa, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

pub struct TransformElement {
    pub(crate) view: TransformView,
    pub(crate) painting: TransformPainting,
}

/// Resolved paint props (filled during layout). Paint reads them directly.
#[derive(Default, Clone)]
pub struct TransformPainting {
    pub(crate) scale: Option<f64>,
    pub(crate) scale_x: Option<f64>,
    pub(crate) scale_y: Option<f64>,
    pub(crate) rotate: Option<f64>,
    pub(crate) translate_x: Option<f64>,
    pub(crate) translate_y: Option<f64>,
    pub(crate) alignment: Alignment,
}

impl Lifecycle for TransformElement {}

impl ElementSubscribe for TransformElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
        if let Some(v) = c.scale.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.scale_x.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.scale_y.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.rotate.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.translate_x.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.translate_y.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.alignment.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for TransformElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(Val::Static(v)) = &self.view.scale {
            parts.push(format!("scale={v}"));
        }
        if let Some(Val::Static(v)) = &self.view.rotate {
            parts.push(format!("rotate={v}"));
        }
        parts.join(" ")
    }
}

impl TransformView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        TransformView {
            scale: p.val::<f64>("scale"),
            scale_x: p.val::<f64>("scaleX"),
            scale_y: p.val::<f64>("scaleY"),
            rotate: p.val::<f64>("rotate"),
            translate_x: p.val::<f64>("translateX"),
            translate_y: p.val::<f64>("translateY"),
            alignment: p.val::<Alignment>("alignment"),
            query_key: p.query_key("queryKey"),
            child: p.child("child"),
        }
    }
}
