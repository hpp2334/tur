use std::rc::Rc;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace};
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

impl OpacityView {
    /// Rut-rail constructor (the `tur-animation` rut rows): a static or
    /// atom-bound opacity around one child.
    pub fn new_rut(value: Option<Val<f32>>, child: Rc<dyn View>) -> Self {
        OpacityView {
            value,
            query_key: Some(vec!["rut".to_string(), "opacity".to_string()]),
            child: Some(child),
        }
    }
}

impl View for OpacityView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(OpacityElement {
                view: self.clone(),
                painting: OpacityPainting::default(),
            }),
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

pub struct OpacityElement {
    pub(crate) view: OpacityView,
    pub(crate) painting: OpacityPainting,
}

impl OpacityElement {
    /// The resolved paint value (layout fills it; tests read it back).
    pub fn painted_value(&self) -> f32 {
        self.painting.value
    }
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

impl TransformView {
    /// Rut-rail constructor (the `tur-animation` rut rows): static
    /// scale / rotate / translate around one child.
    pub fn new_rut(
        scale: Option<Val<f64>>,
        rotate: Option<Val<f64>>,
        translate_x: Option<Val<f64>>,
        translate_y: Option<Val<f64>>,
        child: Rc<dyn View>,
    ) -> Self {
        TransformView {
            scale,
            scale_x: None,
            scale_y: None,
            rotate,
            translate_x,
            translate_y,
            alignment: None,
            query_key: Some(vec!["rut".to_string(), "transform".to_string()]),
            child: Some(child),
        }
    }
}

impl View for TransformView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(TransformElement {
                view: self.clone(),
                painting: TransformPainting::default(),
            }),
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, id.into());
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
