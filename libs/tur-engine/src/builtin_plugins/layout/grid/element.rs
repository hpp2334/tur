use std::rc::Rc;

use boa_engine::Context;
use boa_engine::object::JsObject;

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::js_runtime::JsProps;
use crate::core::layout::{Constraints, Size};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::view::{Lifecycle, Val, View, ViewCx};

use super::GridMetrics;

// ---------------------------------------------------------------------------
// GridView — the user's declaration. Pure Rust, no JsValues.
//
// `max_cross_axis_extent` is reactive and required. The sizing/spacing props
// are optional reactives. Children are a static `Vec<Rc<dyn View>>`.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct GridView {
    pub(crate) max_cross_axis_extent: Val<f64>,
    pub(crate) child_aspect_ratio: Option<Val<f64>>,
    pub(crate) main_axis_extent: Option<Val<f64>>,
    pub(crate) cross_axis_spacing: Option<Val<f64>>,
    pub(crate) main_axis_spacing: Option<Val<f64>>,
    pub(crate) children: Vec<Rc<dyn View>>,
    pub(crate) query_key: Option<Vec<String>>,
}

impl View for GridView {
    fn build(
        &self,
        cx: &mut dyn ViewCx,
        boa: &mut Context,
        parent: crate::core::element::NodeId,
    ) -> crate::core::element::NodeId {
        let id: ElementNodeId = cx.alloc_node().as_element_id();
        cx.insert_node(
            id,
            AnyElement::new(GridElement {
                view: self.clone(),
                metrics: None,
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
// GridElement — the built element. Holds its spec plus transient layout state
// used within `perform_layout` (computed metrics + overflow for clipping).
// ---------------------------------------------------------------------------

pub struct GridElement {
    pub(crate) view: GridView,
    pub(crate) metrics: Option<GridMetrics>,
    pub(crate) constraints: Option<Constraints>,
    pub(crate) computed_size: Option<Size>,
    pub(crate) overflow: f64,
}

impl GridElement {
    /// The cross-axis cell count computed during the last layout (column
    /// count for a vertical-flow grid). 0 before the first layout pass.
    pub fn cross_axis_count(&self) -> usize {
        self.metrics.map(|m| m.cross_axis_count).unwrap_or(0)
    }
}

impl Lifecycle for GridElement {}

impl ElementSubscribe for GridElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
        cx.subscribe_val(&c.max_cross_axis_extent);
        if let Some(v) = c.child_aspect_ratio.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.main_axis_extent.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.cross_axis_spacing.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.main_axis_spacing.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for GridElement {
    fn trace_label(&self) -> String {
        if let Some(m) = &self.metrics {
            format!(
                "cols={} cell={}x{}",
                m.cross_axis_count, m.cell_cross, m.cell_main
            )
        } else {
            "unlaid".to_string()
        }
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let mut p = Vec::new();
        if let Some(v) = self.view.max_cross_axis_extent.as_static() {
            p.push(("maxCrossAxisExtent", TraceValue::Num(*v)));
        }
        if let Some(m) = &self.metrics {
            p.push(("cols", TraceValue::Num(m.cross_axis_count as f64)));
            p.push(("cellCross", TraceValue::Num(m.cell_cross)));
            p.push(("cellMain", TraceValue::Num(m.cell_main)));
        }
        p
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl GridView {
    /// Build a `GridView` from a JS props object. Returns `None` when the
    /// required `maxCrossAxisExtent` prop is missing.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let mut p = JsProps::new(props, ctx);
        let max_cross_axis_extent = p.val::<f64>("maxCrossAxisExtent")?;
        Some(GridView {
            max_cross_axis_extent,
            child_aspect_ratio: p.val::<f64>("childAspectRatio"),
            main_axis_extent: p.val::<f64>("mainAxisExtent"),
            cross_axis_spacing: p.val::<f64>("crossAxisSpacing"),
            main_axis_spacing: p.val::<f64>("mainAxisSpacing"),
            children: p.children("children"),
            query_key: p.query_key("queryKey"),
        })
    }
}
