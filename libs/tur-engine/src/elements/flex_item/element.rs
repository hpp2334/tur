use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::widget::{extract_spec, val_from_js, Effect, PropValue, Spec, Val, WidgetCx};

// ---------------------------------------------------------------------------
// ExpandedSpec — declares a flex item. Has exactly one child; the parent Flex
// detects it via the `tur_flex_item` type name and allocates remaining space.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ExpandedSpec {
    pub flex: Option<Val<f64>>,
    pub child: Rc<dyn Spec>,
}

impl Spec for ExpandedSpec {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();
        cx.insert_node(id, AnyElement::new(Expanded { spec: self.clone() }), boa);
        let _child_id = self.child.build(cx, boa, id);
        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// Expanded — the built element. Passes constraints straight through to its
// single child; the layout contribution (flex space) is decided by the parent.
// ---------------------------------------------------------------------------

pub struct Expanded {
    pub spec: ExpandedSpec,
}

impl Effect for Expanded {}

impl ElementTrace for Expanded {
    fn trace_label(&self) -> String {
        match &self.spec.flex {
            Some(Val::Static(f)) => format!("flex={f}"),
            _ => String::from("flex"),
        }
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

impl ExpandedSpec {
    /// Build an `ExpandedSpec` from a JS props object. Returns `None` when the
    /// required `child` prop is missing.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let child = prop_child(props, "child", ctx)?;
        Some(ExpandedSpec {
            flex: prop_val::<f64>(props, "flex", ctx),
            child,
        })
    }
}
