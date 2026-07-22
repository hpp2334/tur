use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;
use crate::core::layout::{Axis, Constraints, CrossAxisAlignment, MainAxisAlignment, MainAxisSize, Size};

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::js_runtime::JsProps;
use crate::core::view::{ViewCx, Lifecycle, Val, View};

pub struct ChildData {
    pub(crate) id: ElementNodeId,
    pub(crate) size: Size,
    pub(crate) is_flex: bool,
    pub(crate) flex: f64,
}

// ---------------------------------------------------------------------------
// FlexView — the user's declaration. Pure Rust, no JsValues.
//
// `direction` is static (chosen by the factory: Vertical for Column, Horizontal
// for Row) and therefore stored as a plain `Axis` rather than a `Val<Axis>`.
// The alignment / sizing props are reactive.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FlexView {
    pub(crate) direction: Option<Axis>,
    pub(crate) main_alignment: Option<Val<MainAxisAlignment>>,
    pub(crate) cross_alignment: Option<Val<CrossAxisAlignment>>,
    pub(crate) main_axis_size: Option<Val<MainAxisSize>>,
    pub(crate) children: Vec<Rc<dyn View>>,
    pub(crate) query_key: Option<Vec<String>>,
}

impl View for FlexView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(FlexElement {
                view: self.clone(),
                child_data: Vec::new(),
                constraints: None,
                computed_size: None,
                overflow: 0.0,
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
// FlexElement — the built element. Holds its spec plus transient layout state
// used within `perform_layout` (measuring children, then assigning offsets).
// ---------------------------------------------------------------------------

pub struct FlexElement {
    pub(crate) view: FlexView,
    pub(crate) child_data: Vec<ChildData>,
    pub(crate) constraints: Option<Constraints>,
    pub(crate) computed_size: Option<Size>,
    pub(crate) overflow: f64,
}

impl Lifecycle for FlexElement {}

impl ElementSubscribe for FlexElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
        if let Some(v) = c.main_alignment.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.cross_alignment.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.main_axis_size.as_ref() { cx.subscribe_val(v); }
    }
}

impl ElementTrace for FlexElement {
    fn trace_label(&self) -> String {
        format!("{:?}", self.view.direction.unwrap_or(Axis::Vertical))
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let c = &self.view;
        let mut p = Vec::new();
        if let Some(d) = c.direction {
            p.push(("direction", TraceValue::Str(format!("{d:?}"))));
        }
        if let Some(v) = c.main_alignment.as_ref().and_then(Val::as_static) {
            p.push(("mainAlignment", TraceValue::Str(format!("{v:?}"))));
        }
        if let Some(v) = c.cross_alignment.as_ref().and_then(Val::as_static) {
            p.push(("crossAlignment", TraceValue::Str(format!("{v:?}"))));
        }
        if let Some(v) = c.main_axis_size.as_ref().and_then(Val::as_static) {
            p.push(("mainAxisSize", TraceValue::Str(format!("{v:?}"))));
        }
        p
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl FlexView {
    /// Build a `FlexView` from a JS props object. `direction` is supplied by
    /// the factory (`Axis::Vertical` for Column, `Axis::Horizontal` for Row).
    pub fn from_js(direction: Axis, props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        FlexView {
            direction: Some(direction),
            main_alignment: p.val::<MainAxisAlignment>("mainAlignment"),
            cross_alignment: p.val::<CrossAxisAlignment>("crossAlignment"),
            main_axis_size: p.val::<MainAxisSize>("mainAxisSize"),
            children: p.children("children"),
            query_key: p.query_key("queryKey"),
        }
    }
}
