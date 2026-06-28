use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::element::NodeId;
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::widget::{extract_component, val_from_js, Effect, PropValue, Component, Val, WidgetCx};

// ---------------------------------------------------------------------------
// PositionedComponent — the user's declaration. Pure Rust, no JsValues.
//
// A PositionedElement child of a StackElement is placed at the given edges / size. EachElement axis
// is independent: an explicit `width`/`height` wins; otherwise a pair of
// opposing edges (`left`+`right` or `top`+`bottom`) implies a tight extent;
// otherwise that axis is left loose.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PositionedComponent {
    pub left: Option<Val<f64>>,
    pub top: Option<Val<f64>>,
    pub right: Option<Val<f64>>,
    pub bottom: Option<Val<f64>>,
    pub width: Option<Val<f64>>,
    pub height: Option<Val<f64>>,
    pub child: Rc<dyn Component>,
}

impl Component for PositionedComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id = cx.alloc_node();
        cx.insert_node(id, AnyElement::new(PositionedElement { component: self.clone() }), boa);
        let _child_id = self.child.build(cx, boa, id);
        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// PositionedElement — the built element. Offsets its single child by `left`/`top`
// relative to the StackElement's origin.
// ---------------------------------------------------------------------------

pub struct PositionedElement {
    pub component: PositionedComponent,
}

impl Effect for PositionedElement {}

impl ElementSubscribe for PositionedElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.component;
        if let Some(v) = c.left.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.top.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.right.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.bottom.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.width.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.height.as_ref() { cx.subscribe_val(v); }
    }
}

impl ElementTrace for PositionedElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        for (key, val) in [
            ("left", &self.component.left),
            ("top", &self.component.top),
            ("right", &self.component.right),
            ("bottom", &self.component.bottom),
            ("width", &self.component.width),
            ("height", &self.component.height),
        ] {
            if let Some(Val::Static(v)) = val {
                parts.push(format!("{key}={v}"));
            }
        }
        parts.join(" ")
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let c = &self.component;
        let mut p = Vec::new();
        for (key, val) in [
            ("left", &c.left),
            ("top", &c.top),
            ("right", &c.right),
            ("bottom", &c.bottom),
            ("width", &c.width),
            ("height", &c.height),
        ] {
            if let Some(v) = val.as_ref().and_then(Val::as_static) {
                p.push((key, TraceValue::Num(*v)));
            }
        }
        p
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

/// Extract a `Val<T>` prop from a JS props object.
fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

/// Extract the single child spec from a JS props object.
fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn Component>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_component(&v)
}

impl PositionedComponent {
    /// Build a `PositionedComponent` from a JS props object. Returns `None` when
    /// the required `child` prop is missing.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let child = prop_child(props, "child", ctx)?;
        Some(PositionedComponent {
            left: prop_val::<f64>(props, "left", ctx),
            top: prop_val::<f64>(props, "top", ctx),
            right: prop_val::<f64>(props, "right", ctx),
            bottom: prop_val::<f64>(props, "bottom", ctx),
            width: prop_val::<f64>(props, "width", ctx),
            height: prop_val::<f64>(props, "height", ctx),
            child,
        })
    }
}
