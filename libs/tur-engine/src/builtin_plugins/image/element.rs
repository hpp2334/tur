use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;
use crate::core::layout::BoxFit;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::js_runtime::JsProps;
use crate::core::view::{ViewCx, Lifecycle, Val, View};

// ---------------------------------------------------------------------------
// ImageView — the user's declaration. Pure Rust, no JsValues.
//
// `resource_id`, `width`, `height`, and `fit` are reactive (`Val<T>`).
// An optional `child` is supported (rendered behind/over the image — painted
// after the image draw, matching the old behaviour where children render on
// top).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ImageView {
    pub(crate) resource_id: Option<Val<u64>>,
    pub(crate) width: Option<Val<f64>>,
    pub(crate) height: Option<Val<f64>>,
    pub(crate) fit: Option<Val<BoxFit>>,
    pub(crate) query_key: Option<Vec<String>>,
    pub(crate) child: Option<Rc<dyn View>>,
}

impl View for ImageView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(id, AnyElement::new(ImageElement { view: self.clone(), painting: ImagePainting::default() }), boa);
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, boa, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// ImageElement — the built element. Layout and paint read `Val<T>` props on demand.
// ---------------------------------------------------------------------------

/// Resolved paint props (filled during layout). Paint reads these directly.
#[derive(Default, Clone)]
pub struct ImagePainting {
    pub(crate) resource_id: Option<u64>,
    pub(crate) fit: Option<crate::core::layout::BoxFit>,
}

pub struct ImageElement {
    pub(crate) view: ImageView,
    pub(crate) painting: ImagePainting,
}

impl Lifecycle for ImageElement {}

impl ElementSubscribe for ImageElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
        if let Some(v) = c.resource_id.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.fit.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.width.as_ref() { cx.subscribe_val(v); }
        if let Some(v) = c.height.as_ref() { cx.subscribe_val(v); }
    }
}

impl ElementTrace for ImageElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(Val::Static(rid)) = &self.view.resource_id {
            parts.push(format!("resource={rid}"));
        }
        if let Some(Val::Static(w)) = &self.view.width {
            parts.push(format!("width={w}"));
        }
        if let Some(Val::Static(h)) = &self.view.height {
            parts.push(format!("height={h}"));
        }
        if let Some(Val::Static(f)) = &self.view.fit {
            parts.push(format!("fit={f:?}"));
        }
        parts.join(" ")
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let c = &self.view;
        let mut p = Vec::new();
        if let Some(v) = c.resource_id.as_ref().and_then(Val::as_static) {
            p.push(("resourceId", TraceValue::Num(*v as f64)));
        }
        if let Some(v) = c.width.as_ref().and_then(Val::as_static) {
            p.push(("width", TraceValue::Num(*v)));
        }
        if let Some(v) = c.height.as_ref().and_then(Val::as_static) {
            p.push(("height", TraceValue::Num(*v)));
        }
        if let Some(v) = c.fit.as_ref().and_then(Val::as_static) {
            p.push(("fit", TraceValue::Str(format!("{v:?}"))));
        }
        p
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl ImageView {
    /// Build an `ImageView` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        ImageView {
            resource_id: p.val::<u64>("resourceId"),
            width: p.val::<f64>("width"),
            height: p.val::<f64>("height"),
            fit: p.val::<BoxFit>("fit"),
            query_key: p.query_key("queryKey"),
            child: p.child("child"),
        }
    }
}
