use std::rc::Rc;

use crate::core::layout::{Alignment, Size, StackFit};
use boa_engine::Context;
use boa_engine::object::JsObject;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::js_runtime::JsProps;
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::view::{Lifecycle, Val, View, ViewCx};

// ---------------------------------------------------------------------------
// StackView — the user's declaration. Pure Rust, no JsValues.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct StackView {
    pub(crate) fit: Option<Val<StackFit>>,
    pub(crate) alignment: Option<Val<Alignment>>,
    pub(crate) children: Vec<Rc<dyn View>>,
    pub(crate) query_key: Option<Vec<String>>,
}

impl View for StackView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(StackElement {
                view: self.clone(),
                computed_size: None,
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
// StackElement — the built element. Layers its non-positioned children using
// `alignment`; children wrapped in `PositionedElement` (type name `tur_positioned`)
// place themselves.
// ---------------------------------------------------------------------------

pub struct StackElement {
    pub(crate) view: StackView,
    pub(crate) computed_size: Option<Size>,
}

impl Lifecycle for StackElement {}

impl ElementSubscribe for StackElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
        if let Some(v) = c.fit.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.alignment.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for StackElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(Val::Static(f)) = &self.view.fit {
            parts.push(format!("fit={f:?}"));
        }
        if let Some(Val::Static(a)) = &self.view.alignment {
            parts.push(format!("alignment={a:?}"));
        }
        parts.join(" ")
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let c = &self.view;
        let mut p = Vec::new();
        if let Some(v) = c.fit.as_ref().and_then(Val::as_static) {
            p.push(("fit", TraceValue::Str(format!("{v:?}"))));
        }
        if let Some(v) = c.alignment.as_ref().and_then(Val::as_static) {
            p.push(("alignment", TraceValue::Str(format!("{v:?}"))));
        }
        p
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl StackView {
    /// Build a `StackView` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        StackView {
            fit: p.val::<StackFit>("fit"),
            alignment: p.val::<Alignment>("alignment"),
            children: p.children("children"),
            query_key: p.query_key("queryKey"),
        }
    }
}
