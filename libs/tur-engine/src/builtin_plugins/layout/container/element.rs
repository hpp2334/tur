use std::rc::Rc;

use crate::core::layout::{Alignment, BorderPosition, ClipBehavior};
use crate::core::render::brush::{Brush, Color};
use boa_engine::Context;
use boa_engine::object::JsObject;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::js_runtime::JsProps;
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::view::{Lifecycle, Val, View, ViewCx};

// ---------------------------------------------------------------------------
// ContainerView — the user's declaration. Pure Rust, no JsValues.
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct ContainerView {
    pub width: Option<Val<f64>>,
    pub height: Option<Val<f64>>,
    pub padding: Option<Val<f64>>,
    pub color: Option<Val<Brush>>,
    pub border_color: Option<Val<Color>>,
    pub border_width: Option<Val<f64>>,
    pub border_radius: Option<Val<f64>>,
    pub border_position: Option<Val<BorderPosition>>,
    pub clip_behavior: Option<Val<ClipBehavior>>,
    pub shadow_color: Option<Val<Color>>,
    pub shadow_blur: Option<Val<f64>>,
    pub alignment: Option<Val<Alignment>>,
    /// shadowOffset is `[x, y]` — parsed at factory time (not reactive).
    pub shadow_offset: Option<(f64, f64)>,
    pub query_key: Option<Vec<String>>,
    pub children: Vec<Rc<dyn View>>,
}

impl View for ContainerView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(ContainerElement {
                view: self.clone(),
                painting: ContainerPainting::default(),
            }),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        for child_spec in &self.children {
            let _child_id = child_spec.build(cx, boa, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// ContainerElement — the built element. Holds its spec; layout resolves all
// reactive paint props into `painting` (no store access at paint time).
// ---------------------------------------------------------------------------

/// Resolved (concrete) values needed by paint, filled during layout. Paint
/// reads these fields directly and never touches the reactive store.
#[derive(Default, Clone)]
pub struct ContainerPainting {
    pub(crate) shadow_blur: Option<f64>,
    pub(crate) shadow_color: Option<Color>,
    pub(crate) color: Option<Brush>,
    pub(crate) border_color: Option<Color>,
    pub(crate) border_width: Option<f64>,
    pub(crate) border_radius: Option<f64>,
    pub(crate) border_position: BorderPosition,
    pub(crate) clip_behavior: ClipBehavior,
}

pub struct ContainerElement {
    pub(crate) view: ContainerView,
    pub(crate) painting: ContainerPainting,
}

fn static_f64(val: &Option<Val<f64>>) -> Option<f64> {
    match val {
        Some(Val::Static(v)) => Some(*v),
        _ => None,
    }
}

impl ContainerElement {
    pub fn width(&self) -> Option<f64> {
        static_f64(&self.view.width)
    }
    pub fn height(&self) -> Option<f64> {
        static_f64(&self.view.height)
    }
    pub fn padding(&self) -> Option<f64> {
        static_f64(&self.view.padding)
    }
    pub fn border_width(&self) -> Option<f64> {
        static_f64(&self.view.border_width)
    }
    pub fn border_radius(&self) -> Option<f64> {
        static_f64(&self.view.border_radius)
    }
    pub fn shadow_blur(&self) -> Option<f64> {
        static_f64(&self.view.shadow_blur)
    }
    pub fn color(&self) -> Option<Brush> {
        match &self.view.color {
            Some(Val::Static(v)) => Some(v.clone()),
            _ => self.painting.color.clone(),
        }
    }
    pub fn border_color(&self) -> Option<Color> {
        match &self.view.border_color {
            Some(Val::Static(v)) => Some(*v),
            _ => self.painting.border_color,
        }
    }
    pub fn shadow_color(&self) -> Option<Color> {
        match &self.view.shadow_color {
            Some(Val::Static(v)) => Some(*v),
            _ => None,
        }
    }
    pub fn shadow_offset(&self) -> Option<(f64, f64)> {
        self.view.shadow_offset
    }
    pub fn border_position(&self) -> BorderPosition {
        match &self.view.border_position {
            Some(Val::Static(v)) => *v,
            _ => BorderPosition::default(),
        }
    }
}

impl Lifecycle for ContainerElement {}

impl ElementSubscribe for ContainerElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
        if let Some(v) = c.width.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.height.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.padding.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.alignment.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.color.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.border_color.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.border_width.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.border_radius.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.border_position.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.clip_behavior.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.shadow_color.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.shadow_blur.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for ContainerElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(w) = self.view.width.as_ref().and_then(|v| match v {
            Val::Static(f) => Some(*f),
            _ => None,
        }) {
            parts.push(format!("width={w}"));
        }
        if let Some(h) = self.view.height.as_ref().and_then(|v| match v {
            Val::Static(f) => Some(*f),
            _ => None,
        }) {
            parts.push(format!("height={h}"));
        }
        parts.join(" ")
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let c = &self.view;
        let mut p = Vec::new();
        if let Some(v) = c.width.as_ref().and_then(Val::as_static) {
            p.push(("width", TraceValue::Num(*v)));
        }
        if let Some(v) = c.height.as_ref().and_then(Val::as_static) {
            p.push(("height", TraceValue::Num(*v)));
        }
        if let Some(v) = c.padding.as_ref().and_then(Val::as_static) {
            p.push(("padding", TraceValue::Num(*v)));
        }
        if let Some(v) = c.border_width.as_ref().and_then(Val::as_static) {
            p.push(("borderWidth", TraceValue::Num(*v)));
        }
        if let Some(v) = c.border_radius.as_ref().and_then(Val::as_static) {
            p.push(("borderRadius", TraceValue::Num(*v)));
        }
        if let Some(v) = c.shadow_blur.as_ref().and_then(Val::as_static) {
            p.push(("shadowBlur", TraceValue::Num(*v)));
        }
        if let Some(v) = c.alignment.as_ref().and_then(Val::as_static) {
            p.push(("alignment", TraceValue::Str(format!("{v:?}"))));
        }
        if let Some(v) = c.border_position.as_ref().and_then(Val::as_static) {
            p.push(("borderPosition", TraceValue::Str(format!("{v:?}"))));
        }
        p
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl ContainerView {
    /// Build a `ContainerView` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        ContainerView {
            width: p.val::<f64>("width"),
            height: p.val::<f64>("height"),
            padding: p.val::<f64>("padding"),
            color: p.val::<Brush>("color"),
            border_color: p.val::<Color>("borderColor"),
            border_width: p.val::<f64>("borderWidth"),
            border_radius: p.val::<f64>("borderRadius"),
            border_position: p.val::<BorderPosition>("borderPosition"),
            clip_behavior: p.val::<ClipBehavior>("clipBehavior"),
            shadow_color: p.val::<Color>("shadowColor"),
            shadow_blur: p.val::<f64>("shadowBlur"),
            alignment: p.val::<Alignment>("alignment"),
            shadow_offset: p.offset("shadowOffset"),
            query_key: p.query_key("queryKey"),
            children: p.children("children"),
        }
    }
}
