use std::rc::Rc;

use boa_engine::object::builtins::{JsArray, JsFunction};
use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::bridge::JsProps;
use crate::core::element::{FragmentNodeId, NodeId};
use crate::core::elements::{FragmentHost, FragmentKind, TraceValue};
use crate::core::layout::SubscribeCx;
use crate::core::reactive::AnyReadable;
use crate::core::view::{ViewCx, read_atom_raw, extract_view, View};

// ---------------------------------------------------------------------------
// EachView — render one child per item of a reactive array.
//
// `items` is an atom (source or derived) holding a JS array. `build` is a JS
// function `(item, index) => Element` invoked once per item to produce
// that item's subtree. Whenever the `items` atom changes, the mounted item
// subtrees are rebuilt.
//
// EachView is a **fragment**: it hosts its item subtrees in the tree, but
// the enclosing flex lays those items out directly as its own children —
// inheriting the parent's axis and sizing. So an `Each` inside a `Row` flows
// horizontally and an `Each` inside a `Column` flows vertically, both
// content-sized, with no greedy fill.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct EachView {
    items: AnyReadable,
    build: JsFunction,
    query_key: Option<Vec<String>>,
}

/// Invoke the JS `build(item, index)` closure, returning the produced spec.
fn build_item_spec(
    builder: &JsFunction,
    item: &JsValue,
    index: u64,
    boa: &mut Context,
) -> Option<Rc<dyn View>> {
    let result = builder
        .call(
            &JsValue::undefined(),
            &[item.clone(), JsValue::from(index as f64)],
            boa,
        )
        .ok()?;
    extract_view(&result)
}

impl EachView {
    /// Read the current `items` array from the store and build one child per
    /// entry under `fragment_id`. Returns the built children in array order.
    fn build_items(
        &self,
        cx: &mut dyn ViewCx,
        boa: &mut Context,
        fragment_id: FragmentNodeId,
    ) -> Vec<NodeId> {
        let raw = read_atom_raw(cx, self.items, boa);
        let Some(arr) = raw.as_object().and_then(|o| JsArray::from_object(o.clone()).ok()) else {
            return Vec::new();
        };
        let len = arr.length(boa).unwrap_or(0);

        let mut out = Vec::with_capacity(len as usize);
        for i in 0..len as i64 {
            let Ok(item) = arr.at(i, boa) else {
                continue;
            };
            let Some(spec) = build_item_spec(&self.build, &item, i as u64, boa) else {
                continue;
            };
            out.push(spec.build(cx, boa, NodeId::from(fragment_id)));
        }
        out
    }
}

impl View for EachView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id = cx.alloc_node();
        let frag_id = FragmentNodeId::new(id.as_u64());

        let kind = EachFragment {
            view: self.clone(),
        };

        // Register the fragment's reactive deps in the subscriber graph.
        {
            let mut sub_cx = cx.subscribe_fragment(frag_id);
            kind.subscribe(&mut sub_cx);
        }

        // Insert the empty fragment FIRST so items can auto-link to it
        // via `append_child` (which pushes to `frag.children`).
        let host = FragmentHost {
            id: frag_id,
            parent,
            children: Vec::new(),
            kind: Some(Box::new(kind)),
            query_key: self.query_key.clone(),
        };
        cx.insert_fragment(host);

        // Build items under `frag_id` — each auto-links to the fragment.
        self.build_items(cx, boa, frag_id);

        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// EachFragment — the `FragmentKind` impl. Rebuilds all items when `items`
// atom changes.
// ---------------------------------------------------------------------------

pub struct EachFragment {
    view: EachView,
}

impl FragmentKind for EachFragment {
    fn type_name(&self) -> &'static str {
        "tur_each"
    }

    fn trace_label(&self, children: &[NodeId]) -> String {
        format!("items={}", children.len())
    }

    fn trace_props(&self, children: &[NodeId]) -> Vec<(&'static str, TraceValue)> {
        vec![("itemCount", TraceValue::Num(children.len() as f64))]
    }

    fn subscribe(&self, cx: &mut SubscribeCx) {
        cx.subscribe_readable(self.view.items);
    }

    fn perform_update(
        &mut self,
        cx: &mut dyn ViewCx,
        boa: &mut Context,
        fragment_id: FragmentNodeId,
    ) -> Option<Vec<NodeId>> {
        // Rebuild-all reconciliation: tear down every previously mounted item
        // and rebuild from the current array. Simple and correct; the item
        // subtrees are stateless widgets so rebuilding them is cheap.
        Some(self.view.build_items(cx, boa, fragment_id))
    }
}

// ---------------------------------------------------------------------------
// Factory — parse props into a spec.
// ---------------------------------------------------------------------------

impl EachView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Option<Self> {
        let mut p = JsProps::new(props, ctx);
        Some(EachView {
            items: p.readable("items")?,
            build: p.function("build")?,
            query_key: p.query_key("queryKey"),
        })
    }
}
