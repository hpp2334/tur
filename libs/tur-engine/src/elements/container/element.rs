use std::rc::Rc;

use boa_engine::Context;
use tur_shared::{Alignment, BorderPosition, Brush, Color};

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::widget::{
    val_from_js, Effect, PropValue, Component, Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// ContainerComponent — the user's declaration. Pure Rust, no JsValues.
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct ContainerComponent {
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
    pub children: Vec<Rc<dyn Component>>,
}

impl Component for ContainerComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();
        cx.insert_node(
            id,
            AnyElement::new(ContainerElement { component: self.clone(), cached_color: None, cached_border_color: None }),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        for child_spec in &self.children {
            let _child_id = child_spec.build(cx, boa, id);
        }
        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// ContainerElement — the built element. Holds its spec; layout/paint read Val<T>
// on demand via `cx.read_val`.
// ---------------------------------------------------------------------------

pub struct ContainerElement {
    pub component: ContainerComponent,
    pub cached_color: Option<Brush>,
    pub cached_border_color: Option<Color>,
}

fn static_f64(val: &Option<Val<f64>>) -> Option<f64> {
    match val {
        Some(Val::Static(v)) => Some(*v),
        _ => None,
    }
}

impl ContainerElement {
    pub fn width(&self) -> Option<f64> { static_f64(&self.component.width) }
    pub fn height(&self) -> Option<f64> { static_f64(&self.component.height) }
    pub fn padding(&self) -> Option<f64> { static_f64(&self.component.padding) }
    pub fn border_width(&self) -> Option<f64> { static_f64(&self.component.border_width) }
    pub fn border_radius(&self) -> Option<f64> { static_f64(&self.component.border_radius) }
    pub fn shadow_blur(&self) -> Option<f64> { static_f64(&self.component.shadow_blur) }
    pub fn color(&self) -> Option<Brush> {
        match &self.component.color {
            Some(Val::Static(v)) => Some(v.clone()),
            _ => self.cached_color.clone(),
        }
    }
    pub fn border_color(&self) -> Option<Color> {
        match &self.component.border_color {
            Some(Val::Static(v)) => Some(*v),
            _ => self.cached_border_color,
        }
    }
    pub fn shadow_color(&self) -> Option<Color> {
        match &self.component.shadow_color { Some(Val::Static(v)) => Some(*v), _ => None }
    }
    pub fn shadow_offset(&self) -> Option<(f64, f64)> { self.component.shadow_offset }
    pub fn border_position(&self) -> BorderPosition {
        match &self.component.border_position {
            Some(Val::Static(v)) => *v,
            _ => BorderPosition::default(),
        }
    }
}

impl Effect for ContainerElement {}

impl ElementTrace for ContainerElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(w) = self.component.width.as_ref().and_then(|v| match v {
            Val::Static(f) => Some(*f),
            _ => None,
        }) {
            parts.push(format!("width={w}"));
        }
        if let Some(h) = self.component.height.as_ref().and_then(|v| match v {
            Val::Static(f) => Some(*f),
            _ => None,
        }) {
            parts.push(format!("height={h}"));
        }
        parts.join(" ")
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let c = &self.component;
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
) -> Vec<Rc<dyn Component>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    use crate::core::widget::extract_component;
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
            if let Some(spec) = extract_component(&item) {
                out.push(spec);
            }
        }
    }
    out
}

impl ContainerComponent {
    /// Build a `ContainerComponent` from a JS props object.
    pub fn from_js(props: &boa_engine::object::JsObject, ctx: &mut Context) -> Self {
        ContainerComponent {
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
