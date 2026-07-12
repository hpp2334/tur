use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::bridge::JsProps;
use crate::core::view::{ViewCx, Lifecycle, Val, View};

// ---------------------------------------------------------------------------
// ExpandedView — declares a flex item. Has exactly one child; the parent FlexElement
// detects it via the `tur_flex_item` type name and allocates remaining space.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ExpandedView {
    pub(crate) flex: Option<Val<f64>>,
    child: Rc<dyn View>,
}

impl View for ExpandedView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(id, AnyElement::new(ExpandedElement { view: self.clone() }), boa);
        let _child_id = self.child.build(cx, boa, id.into());
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// ExpandedElement — the built element. Passes constraints straight through to its
// single child; the layout contribution (flex space) is decided by the parent.
// ---------------------------------------------------------------------------

pub struct ExpandedElement {
    pub(crate) view: ExpandedView,
}

impl Lifecycle for ExpandedElement {}

impl ElementSubscribe for ExpandedElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        // The flex prop is read by the parent via `child_flex`, but declaring
        // it here dirties this node — and `mark_dirty` propagates up to the
        // parent Flex, redistributing flex space.
        if let Some(v) = self.view.flex.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for ExpandedElement {
    fn trace_label(&self) -> String {
        match &self.view.flex {
            Some(Val::Static(f)) => format!("flex={f}"),
            _ => String::from("flex"),
        }
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        self.view
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

impl ExpandedView {
    /// Build an `ExpandedView` from a JS props object. Returns `None` when the
    /// required `child` prop is missing.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let mut p = JsProps::new(props, ctx);
        let child = p.child("child")?;
        Some(ExpandedView {
            flex: p.val::<f64>("flex"),
            child,
        })
    }
}
