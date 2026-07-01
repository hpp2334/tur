use std::rc::Rc;

use boa_engine::Context;
use tur_shared::{Alignment, BorderPosition, Brush, Color};

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::view::{
    ViewCx,
    val_from_js, Lifecycle, PropValue, View, Val,
};
use crate::core::layout::{ElementSubscribe, SubscribeCx};

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
            AnyElement::new(ContainerElement { view: self.clone(), painting: ContainerPainting::default() }),
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
    pub shadow_blur: Option<f64>,
    pub shadow_color: Option<Color>,
    pub color: Option<Brush>,
    pub border_color: Option<Color>,
    pub border_width: Option<f64>,
    pub border_radius: Option<f64>,
    pub border_position: BorderPosition,
}

pub struct ContainerElement {
    pub view: ContainerView,
    pub painting: ContainerPainting,
}

fn static_f64(val: &Option<Val<f64>>) -> Option<f64> {
    match val {
        Some(Val::Static(v)) => Some(*v),
        _ => None,
    }
}

impl ContainerElement {
    pub fn width(&self) -> Option<f64> { static_f64(&self.view.width) }
    pub fn height(&self) -> Option<f64> { static_f64(&self.view.height) }
    pub fn padding(&self) -> Option<f64> { static_f64(&self.view.padding) }
    pub fn border_width(&self) -> Option<f64> { static_f64(&self.view.border_width) }
    pub fn border_radius(&self) -> Option<f64> { static_f64(&self.view.border_radius) }
    pub fn shadow_blur(&self) -> Option<f64> { static_f64(&self.view.shadow_blur) }
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
        match &self.view.shadow_color { Some(Val::Static(v)) => Some(*v), _ => None }
    }
    pub fn shadow_offset(&self) -> Option<(f64, f64)> { self.view.shadow_offset }
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
        if let Some(v) = c.width.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.height.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.padding.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.alignment.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.color.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.border_color.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.border_width.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.border_radius.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.border_position.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.shadow_color.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.shadow_blur.as_ref() { cx.subscribe_val(v); }
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

/// Extract a `Val<T>` prop from a JS props object.
fn prop_val<T: PropValue>(
    props: &boa_engine::object::JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

/// Extract a `(f64, f64)` offset prop (shadowOffset) — parsed eagerly.
fn prop_offset(
    props: &boa_engine::object::JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<(f64, f64)> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    let obj = v.as_object()?;
    let arr = JsArray::from_object(obj.clone()).ok()?;
    let x = arr.at(0, ctx).ok()?.as_number()?;
    let y = arr.at(1, ctx).ok()?.as_number()?;
    Some((x, y))
}

/// Extract a `Vec<String>` prop (queryKey) — parsed eagerly.
fn prop_query_key(
    props: &boa_engine::object::JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<Vec<String>> {
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

/// Extract child specs from a JS array of ComponentHandle opaques.
fn prop_children(
    props: &boa_engine::object::JsObject,
    key: &str,
    ctx: &mut Context,
) -> Vec<Rc<dyn View>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    use crate::core::view::extract_view;
    let Ok(v) = props.get(js_string!(key), ctx) else {
        return Vec::new();
    };
    let Some(obj) = v.as_object() else {
        return Vec::new();
    };
    let Ok(arr) = JsArray::from_object(obj.clone()) else {
        return Vec::new();
    };
    let len = arr.length(ctx).unwrap_or(0);
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len {
        if let Ok(item) = arr.at(i as i64, ctx) {
            if let Some(spec) = extract_view(&item) {
                out.push(spec);
            }
        }
    }
    out
}

impl ContainerView {
    /// Build a `ContainerView` from a JS props object.
    pub fn from_js(props: &boa_engine::object::JsObject, ctx: &mut Context) -> Self {
        ContainerView {
            width: prop_val::<f64>(props, "width", ctx),
            height: prop_val::<f64>(props, "height", ctx),
            padding: prop_val::<f64>(props, "padding", ctx),
            color: prop_val::<Brush>(props, "color", ctx),
            border_color: prop_val::<Color>(props, "borderColor", ctx),
            border_width: prop_val::<f64>(props, "borderWidth", ctx),
            border_radius: prop_val::<f64>(props, "borderRadius", ctx),
            border_position: prop_val::<BorderPosition>(props, "borderPosition", ctx),
            shadow_color: prop_val::<Color>(props, "shadowColor", ctx),
            shadow_blur: prop_val::<f64>(props, "shadowBlur", ctx),
            alignment: prop_val::<Alignment>(props, "alignment", ctx),
            shadow_offset: prop_offset(props, "shadowOffset", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
            children: prop_children(props, "children", ctx),
        }
    }
}
