use std::rc::Rc;

use boa_engine::Context;
use boa_engine::object::JsObject;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::js_runtime::JsProps;
use crate::core::layout::{ElementSubscribe, FlexFit, SubscribeCx};
use crate::core::view::{Lifecycle, Val, View, ViewCx};

// ---------------------------------------------------------------------------
// FlexibleView — declares a flex item. Has exactly one child; the parent
// FlexElement detects it via the `tur_flexible` type name and allocates
// remaining main-axis space. `fit` selects how the child is inscribed into
// its slot (Flutter `FlexFit`):
//
// - `Tight` — the child is forced to fill its slot (`Expanded`, which in
//   Flutter is literally `Flexible(fit: FlexFit.tight)`).
// - `Loose` — the child may be at most its slot, but is allowed to be
//   smaller (plain `Flexible`; the child shrink-wraps below the slot).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FlexibleView {
    pub(crate) flex: Option<Val<f64>>,
    pub(crate) fit: FlexFit,
    pub(crate) query_key: Option<Vec<String>>,
    child: Rc<dyn View>,
}

impl View for FlexibleView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(FlexibleElement { view: self.clone() }),
            boa,
        );
        let _child_id = self.child.build(cx, boa, id.into());
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// FlexibleElement — the built element. Passes constraints straight through
// to its single child; the layout contribution (flex space + fit) is
// decided by the parent.
// ---------------------------------------------------------------------------

pub struct FlexibleElement {
    pub(crate) view: FlexibleView,
}

impl Lifecycle for FlexibleElement {}

impl ElementSubscribe for FlexibleElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        // The flex prop is read by the parent via `child_flex`, but declaring
        // it here dirties this node — and `mark_dirty` propagates up to the
        // parent Flex, redistributing flex space.
        if let Some(v) = self.view.flex.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for FlexibleElement {
    fn trace_label(&self) -> String {
        match (&self.view.flex, self.view.fit) {
            (Some(Val::Static(f)), fit) => format!("flex={f} fit={fit:?}"),
            _ => String::from("flex"),
        }
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        self.view
            .flex
            .as_ref()
            .and_then(Val::as_static)
            .map(|f| {
                vec![
                    ("flex", TraceValue::Num(*f)),
                    ("fit", TraceValue::Str(format!("{:?}", self.view.fit))),
                ]
            })
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl FlexibleView {
    /// Build a `FlexibleView` from a JS props object. `default_fit` is the
    /// constructor's fit (`Expanded` → `Tight`, `Flexible` → `Loose`); an
    /// explicit `fit` prop (e.g. `Flexible().fit(FlexFit.Tight)`) overrides
    /// it. Returns `None` when the required `child` prop is missing.
    ///
    /// `fit` is static-only (Flutter's `fit` is a constructor parameter, not
    /// a reactive prop): a `Val::Reactive` fit is ignored in favor of
    /// `default_fit`.
    pub fn from_js(props: &JsObject, ctx: &mut Context, default_fit: FlexFit) -> Option<Self> {
        let mut p = JsProps::new(props, ctx);
        let child = p.child("child")?;
        Some(FlexibleView {
            flex: p.val::<f64>("flex"),
            fit: p
                .val::<FlexFit>("fit")
                .and_then(|v| v.as_static().copied())
                .unwrap_or(default_fit),
            query_key: p.query_key("queryKey"),
            child,
        })
    }
}
