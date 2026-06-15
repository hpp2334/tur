use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::widget::{extract_spec, val_from_js, Effect, PropValue, Spec, Val, WidgetCx};

// ---------------------------------------------------------------------------
// PositionedSpec — the user's declaration. Pure Rust, no JsValues.
//
// A Positioned child of a Stack is placed at the given edges / size. Each axis
// is independent: an explicit `width`/`height` wins; otherwise a pair of
// opposing edges (`left`+`right` or `top`+`bottom`) implies a tight extent;
// otherwise that axis is left loose.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PositionedSpec {
    pub left: Option<Val<f64>>,
    pub top: Option<Val<f64>>,
    pub right: Option<Val<f64>>,
    pub bottom: Option<Val<f64>>,
    pub width: Option<Val<f64>>,
    pub height: Option<Val<f64>>,
    pub child: Rc<dyn Spec>,
}

impl Spec for PositionedSpec {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();
        cx.insert_node(id, AnyElement::new(Positioned { spec: self.clone() }), boa);
        let _child_id = self.child.build(cx, boa, id);
        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// Positioned — the built element. Offsets its single child by `left`/`top`
// relative to the Stack's origin.
// ---------------------------------------------------------------------------

pub struct Positioned {
    pub spec: PositionedSpec,
}

impl Effect for Positioned {}

impl ElementTrace for Positioned {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        for (key, val) in [
            ("left", &self.spec.left),
            ("top", &self.spec.top),
            ("right", &self.spec.right),
            ("bottom", &self.spec.bottom),
            ("width", &self.spec.width),
            ("height", &self.spec.height),
        ] {
            if let Some(Val::Static(v)) = val {
                parts.push(format!("{key}={v}"));
            }
        }
        parts.join(" ")
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
fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn Spec>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_spec(&v)
}

impl PositionedSpec {
    /// Build a `PositionedSpec` from a JS props object. Returns `None` when
    /// the required `child` prop is missing.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let child = prop_child(props, "child", ctx)?;
        Some(PositionedSpec {
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
