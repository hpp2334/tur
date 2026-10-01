use std::rc::Rc;

use crate::core::edgy::reactive::AnyReadable;
use crate::core::edgy::value::Value;
use crate::core::element::{FragmentNodeId, NodeId};
use crate::core::elements::{FragmentHost, FragmentKind, TraceValue};
use crate::core::layout::SubscribeCx;
use crate::core::rut_runtime::{RutHandles, VmFace, opaque_to_view};
use crate::core::view::{View, ViewCx};
use rut_vm::OpaqueRef;

// ---------------------------------------------------------------------------
// EachView — render one child per item of a list.
//
// `items` is either a reactive atom (a native `Value::List`) or a static
// list authored at start time. `build` is the rut-native item builder: a
// named `entry fn(index, item) -> opaque` invoked through the guarded VM
// face (fuel-capped, depth-limited, no-mount-guarded, traps reported — the
// same flush-time-call law the `rs_each` row formalizes). Whenever the
// `items` atom changes, the mounted item subtrees are rebuilt.
//
// EachView is a **fragment**: it hosts its item subtrees in the tree, but
// the enclosing flex lays those items out directly as its own children —
// inheriting the parent's axis and sizing. So an `Each` inside a `Row` flows
// horizontally and an `Each` inside a `Column` flows vertically, both
// content-sized, with no greedy fill.
// ---------------------------------------------------------------------------

/// The item-builder entry: a name + everything a guarded face call needs
/// (the row captures `face` / `handles` off the boot handles).
#[derive(Clone)]
pub struct EachBuilder {
    pub name: String,
    pub face: Rc<VmFace>,
    pub handles: Rc<RutHandles>,
}

impl EachBuilder {
    /// Resolve the item spec for `(index, item)`. Items cross the entry
    /// boundary as strings (the C2 gate's shape; structured items ride the
    /// value rows).
    fn build(&self, index: u64, item: &Value) -> Option<Rc<dyn View>> {
        let item_str = item.as_str().unwrap_or_default();
        let handle: Result<OpaqueRef, _> =
            self.face.call(&self.handles, &self.name, (index, item_str));
        // Face calls report their own traps (error rail); a failed item
        // degrades to "not built".
        handle.ok().and_then(|h| opaque_to_view(&h))
    }
}

#[derive(Clone)]
pub struct EachView {
    /// The reactive items atom (a native `Value::List`) — when bound, the
    /// mounted items rebuild on every change.
    items: Option<AnyReadable>,
    /// Static items (no atom bound) — authored at start time.
    items_static: Vec<Value>,
    build: EachBuilder,
    query_key: Option<Vec<String>>,
}

impl EachView {
    /// Rut-rail constructor (`core::rut_runtime`): a reactive list atom or
    /// static items + the entry-fn item builder.
    pub fn new_rut(
        items: Option<AnyReadable>,
        items_static: Vec<Value>,
        build: EachBuilder,
    ) -> Self {
        EachView {
            items,
            items_static,
            build,
            query_key: Some(vec!["rut".to_string(), "each".to_string()]),
        }
    }

    /// Read the current items (the atom when bound, else the static list)
    /// and build one child per entry under `fragment_id`. Returns the built
    /// children in array order.
    fn build_items(&self, cx: &mut dyn ViewCx, fragment_id: FragmentNodeId) -> Vec<NodeId> {
        let Some(atom) = self.items else {
            return self.build_from(cx, &self.items_static, fragment_id);
        };
        let store = cx.store_read_only();
        let value = store.read(atom);
        let Value::List(items) = &value else {
            return Vec::new();
        };
        self.build_from(cx, items, fragment_id)
    }

    /// Build the item specs into the tree under `fragment_id` (each
    /// auto-links to the fragment).
    fn build_from(
        &self,
        cx: &mut dyn ViewCx,
        items: &[Value],
        fragment_id: FragmentNodeId,
    ) -> Vec<NodeId> {
        let mut out = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            if let Some(spec) = self.build.build(index as u64, item) {
                out.push(spec.build(cx, NodeId::from(fragment_id)));
            }
        }
        out
    }
}

impl View for EachView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let id = cx.alloc_node();
        let frag_id = FragmentNodeId::new(id.as_u64());

        let kind = EachFragment { view: self.clone() };

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
        self.build_items(cx, frag_id);

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
        if let Some(atom) = self.view.items {
            cx.subscribe_readable(atom);
        }
    }

    fn perform_update(
        &mut self,
        cx: &mut dyn ViewCx,
        fragment_id: FragmentNodeId,
    ) -> Option<Vec<NodeId>> {
        // Rebuild-all reconciliation: tear down every previously mounted item
        // and rebuild from the current array. Simple and correct; the item
        // subtrees are stateless widgets so rebuilding them is cheap. The
        // builder entries run through the guarded face — flush-safe.
        Some(self.view.build_items(cx, fragment_id))
    }
}
