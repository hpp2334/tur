use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::element::NodeId;
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::widget::{
    extract_component, val_from_js, Effect, PropValue, Component, Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// OpacityComponent — applies an alpha multiplier to its child subtree.
//
// `value` is the opacity in [0.0, 1.0] and is reactive.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct OpacityComponent {
    pub value: Option<Val<f32>>,
    pub query_key: Option<Vec<String>>,
    pub child: Option<Rc<dyn Component>>,
}

impl Component for OpacityComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id = cx.alloc_node();
        cx.insert_node(id, AnyElement::new(OpacityElement { component: self.clone(), painting: OpacityPainting::default() }), boa);
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, boa, id);
        }
        cx.link_child(parent, id);
        id
    }
}

pub struct OpacityElement {
    pub component: OpacityComponent,
    pub painting: OpacityPainting,
}

/// Resolved paint prop (filled during layout). Paint reads it directly.
#[derive(Clone)]
pub struct OpacityPainting {
    pub value: f32,
}
impl Default for OpacityPainting {
    fn default() -> Self {
        Self { value: 1.0 }
    }
}

impl Effect for OpacityElement {}

impl ElementSubscribe for OpacityElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        if let Some(v) = self.component.value.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for OpacityElement {
    fn trace_label(&self) -> String {
        if let Some(Val::Static(v)) = &self.component.value {
            format!("opacity={v}")
        } else {
            String::new()
        }
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

fn prop_query_key(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Vec<String>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    let obj = v.as_object()?;
    let arr = JsArray::from_object(obj.clone()).ok()?;
    let len = arr.length(ctx).ok()? as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if let Ok(val) = arr.at(i as i64, ctx) {
            if let Some(s) = val.as_string() {
                out.push(s.to_std_string_escaped());
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn Component>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_component(&v)
}

impl OpacityComponent {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        OpacityComponent {
            value: prop_val::<f32>(props, "value", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// TransformComponent — applies a 2D affine transform to its child subtree.
//
// Supported props: `scale` (uniform), `scaleX`, `scaleY`, `rotate` (radians),
// `translateX`, `translateY`. All reactive.
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct TransformComponent {
    pub scale: Option<Val<f64>>,
    pub scale_x: Option<Val<f64>>,
    pub scale_y: Option<Val<f64>>,
    pub rotate: Option<Val<f64>>,
    pub translate_x: Option<Val<f64>>,
    pub translate_y: Option<Val<f64>>,
    pub query_key: Option<Vec<String>>,
    pub child: Option<Rc<dyn Component>>,
}

impl Component for TransformComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id = cx.alloc_node();
        cx.insert_node(id, AnyElement::new(TransformElement { component: self.clone(), painting: TransformPainting::default() }), boa);
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, boa, id);
        }
        cx.link_child(parent, id);
        id
    }
}

pub struct TransformElement {
    pub component: TransformComponent,
    pub painting: TransformPainting,
}

/// Resolved paint props (filled during layout). Paint reads them directly.
#[derive(Default, Clone)]
pub struct TransformPainting {
    pub scale: Option<f64>,
    pub scale_x: Option<f64>,
    pub scale_y: Option<f64>,
    pub rotate: Option<f64>,
    pub translate_x: Option<f64>,
    pub translate_y: Option<f64>,
}

impl Effect for TransformElement {}

impl ElementSubscribe for TransformElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.component;
        if let Some(v) = c.scale.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.scale_x.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.scale_y.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.rotate.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.translate_x.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.translate_y.as_ref() { cx.subscribe_val(v); }
    }
}

impl ElementTrace for TransformElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(Val::Static(v)) = &self.component.scale { parts.push(format!("scale={v}")); }
        if let Some(Val::Static(v)) = &self.component.rotate { parts.push(format!("rotate={v}")); }
        parts.join(" ")
    }
}

impl TransformComponent {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        TransformComponent {
            scale: prop_val::<f64>(props, "scale", ctx),
            scale_x: prop_val::<f64>(props, "scaleX", ctx),
            scale_y: prop_val::<f64>(props, "scaleY", ctx),
            rotate: prop_val::<f64>(props, "rotate", ctx),
            translate_x: prop_val::<f64>(props, "translateX", ctx),
            translate_y: prop_val::<f64>(props, "translateY", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}
