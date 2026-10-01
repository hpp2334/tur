use std::rc::Rc;

use crate::core::layout::BoxFit;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::view::{Lifecycle, Val, View, ViewCx};

use super::handle::ImageResourceRef;

// ---------------------------------------------------------------------------
// ImageView — the user's declaration. Pure Rust.
//
// `resource_id`, `width`, `height`, and `fit` are reactive (`Val<T>`).
// An optional `child` is supported (rendered behind/over the image — painted
// after the image draw, matching the old behaviour where children render on
// top).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ImageView {
    pub(crate) resource_id: Option<Val<ImageResourceRef>>,
    pub(crate) width: Option<Val<f64>>,
    pub(crate) height: Option<Val<f64>>,
    pub(crate) fit: Option<Val<BoxFit>>,
    pub(crate) query_key: Option<Vec<String>>,
    pub(crate) child: Option<Rc<dyn View>>,
}

impl View for ImageView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(ImageElement {
                view: self.clone(),
                painting: ImagePainting::default(),
            }),
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// ImageElement — the built element. Layout and paint read `Val<T>` props on demand.
// ---------------------------------------------------------------------------/// Resolved paint props (filled during layout). Paint reads these directly.
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
        if let Some(v) = c.resource_id.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.fit.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.width.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.height.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for ImageElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(Val::Static(rid)) = &self.view.resource_id {
            parts.push(format!("resource={}", rid.0));
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
            p.push(("resourceId", TraceValue::Num(v.0 as f64)));
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
// `ImageView` is authored directly (struct literal) by the rut rows in
// `core::rut_runtime::image_row` — the fields are `pub(crate)` and every
// prop defaults through `Option`/`Val::Static`.
// ---------------------------------------------------------------------------
