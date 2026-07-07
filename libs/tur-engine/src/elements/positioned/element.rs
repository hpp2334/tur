use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::view::{ViewCx, extract_view, val_from_js, Lifecycle, PropValue, View, Val};

// ---------------------------------------------------------------------------
// PositionedView — the user's declaration. Pure Rust, no JsValues.
//
// A PositionedElement child of a StackElement is placed at the given edges / size. EachElement axis
// is independent: an explicit `width`/`height` wins; otherwise a pair of
// opposing edges (`left`+`right` or `top`+`bottom`) implies a tight extent;
// otherwise that axis is left loose.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PositionedView {
    pub(crate) left: Option<Val<f64>>,
    pub(crate) top: Option<Val<f64>>,
    pub(crate) right: Option<Val<f64>>,
    pub(crate) bottom: Option<Val<f64>>,
    pub(crate) width: Option<Val<f64>>,
    pub(crate) height: Option<Val<f64>>,
    pub(crate) child: Rc<dyn View>,
}

impl View for PositionedView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(id, AnyElement::new(PositionedElement { view: self.clone() }), boa);
        let _child_id = self.child.build(cx, boa, id.into());
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// PositionedElement — the built element. Offsets its single child by `left`/`top`
// relative to the StackElement's origin.
// ---------------------------------------------------------------------------

pub struct PositionedElement {
    pub(crate) view: PositionedView,
}

impl Lifecycle for PositionedElement {}

impl ElementSubscribe for PositionedElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
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
            ("left", &self.view.left),
            ("top", &self.view.top),
            ("right", &self.view.right),
            ("bottom", &self.view.bottom),
            ("width", &self.view.width),
            ("height", &self.view.height),
        ] {
            if let Some(Val::Static(v)) = val {
                parts.push(format!("{key}={v}"));
            }
        }
        parts.join(" ")
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let c = &self.view;
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
fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn View>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_view(&v)
}

impl PositionedView {
    /// Build a `PositionedView` from a JS props object. Returns `None` when
    /// the required `child` prop is missing.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let child = prop_child(props, "child", ctx)?;
        Some(PositionedView {
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
