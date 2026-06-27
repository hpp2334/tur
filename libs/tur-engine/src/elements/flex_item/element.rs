use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::widget::{extract_component, val_from_js, Effect, PropValue, Component, Val, WidgetCx};

// ---------------------------------------------------------------------------
// ExpandedComponent — declares a flex item. Has exactly one child; the parent FlexElement
// detects it via the `tur_flex_item` type name and allocates remaining space.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ExpandedComponent {
    pub flex: Option<Val<f64>>,
    pub child: Rc<dyn Component>,
}

impl Component for ExpandedComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();
        cx.insert_node(id, AnyElement::new(ExpandedElement { component: self.clone() }), boa);
        let _child_id = self.child.build(cx, boa, id);
        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// ExpandedElement — the built element. Passes constraints straight through to its
// single child; the layout contribution (flex space) is decided by the parent.
// ---------------------------------------------------------------------------

pub struct ExpandedElement {
    pub component: ExpandedComponent,
}

impl Effect for ExpandedElement {}

impl ElementSubscribe for ExpandedElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        // The flex prop is read by the parent via `child_flex`, but declaring
        // it here dirties this node — and `mark_dirty` propagates up to the
        // parent Flex, redistributing flex space.
        if let Some(v) = self.component.flex.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for ExpandedElement {
    fn trace_label(&self) -> String {
        match &self.component.flex {
            Some(Val::Static(f)) => format!("flex={f}"),
            _ => String::from("flex"),
        }
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        self.component
            .flex
            .as_ref()
            .and_then(Val::as_static)
            .map(|f| vec![("flex", TraceValue::Num(*f))])
            .unwrap_or_default()
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

impl ExpandedComponent {
    /// Build an `ExpandedComponent` from a JS props object. Returns `None` when the
    /// required `child` prop is missing.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let child = prop_child(props, "child", ctx)?;
        Some(ExpandedComponent {
            flex: prop_val::<f64>(props, "flex", ctx),
            child,
        })
    }
}
