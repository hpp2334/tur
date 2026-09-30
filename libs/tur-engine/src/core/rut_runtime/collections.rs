//! C2 — collections: `Each` over a native `Value::List` atom + the
//! lazy-container rows.
//!
//! rut closures cannot cross the host boundary, so the item builder is a
//! named `entry fn` invoked through the guarded VM face ([`VmFace`]) —
//! the same flush-time-call law C8 formalizes for deriveds:
//! fuel-capped, depth-limited, no-mount-guarded, traps reported (never
//! aborting the flush).
//!
//! - `rs_each(list_atom, item_fn)` — one child per item of the list atom;
//!   rebuild-all reconciliation when the atom changes (the JS `Each`
//!   semantics). The item builder signature is
//!   `entry fn item_fn(index: u64, item: str) -> opaque`.
//! - `el_lazy_list(item_fn, count_atom, item_extent)` /
//!   `el_lazy_grid(item_fn, count_atom, max_cross, aspect)` — the
//!   `LazyList` / `LazyGrid` elements with the same builder shape minus the
//!   item payload (lazy rows are addressed by index alone).

use std::rc::Rc;

use crate::builtin_plugins::lazy_container::item_builder::RutEntryBuilder;
use crate::core::edgy::reactive::AnyReadable;
use crate::core::element::{FragmentNodeId, NodeId};
use crate::core::elements::{FragmentHost, FragmentKind, TraceValue};
use crate::core::layout::SubscribeCx;
use crate::core::rut_runtime::{RutHandles, VmFace};
use crate::core::view::{View, ViewCx};
use rut_vm::OpaqueRef;

use super::RutView;

// ---------------------------------------------------------------------------
// RutEachView — the JS `Each` twin over a native list atom.
// ---------------------------------------------------------------------------

/// The item-builder entry: a name + everything a face call needs.
#[derive(Clone)]
struct EachBuilder {
    name: String,
    face: Rc<VmFace>,
    handles: Rc<RutHandles>,
}

impl EachBuilder {
    /// Resolve the item spec for `(index, item)`.
    fn build(&self, index: u64, item: &str) -> Option<Rc<dyn View>> {
        let handle: Result<OpaqueRef, _> = self.face.call(&self.handles, &self.name, (index, item));
        handle.ok().and_then(|h| super::opaque_to_view(&h))
    }
}

/// Render one child per item of a reactive `Value::List` atom — the rut
/// twin of the JS `Each`. A fragment: the enclosing flex lays the item
/// subtrees out directly as its own children.
pub struct RutEachView {
    items: AnyReadable,
    builder: EachBuilder,
    query_key: Option<Vec<String>>,
}

impl RutEachView {
    /// Read the current `items` list from the store and build one child per
    /// entry under `fragment_id`. Items cross the entry boundary as strings
    /// (the C2 gate's shape; structured items ride the value rows).
    fn build_items(&self, cx: &mut dyn ViewCx, fragment_id: FragmentNodeId) -> Vec<NodeId> {
        let store = cx.store_read_only();
        let value = store.read(self.items, None);
        let Value::List(items) = &value else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            let item_str = item.as_str().unwrap_or_default().to_string();
            if let Some(spec) = self.builder.build(index as u64, &item_str) {
                out.push(spec.build(cx, NodeId::from(fragment_id)));
            }
        }
        out
    }
}

use crate::core::edgy::value::Value;

impl View for RutEachView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let id = cx.alloc_node();
        let frag_id = FragmentNodeId::new(id.as_u64());

        let kind = RutEachFragment {
            items: self.items,
            builder: self.builder.clone(),
        };

        // Register the fragment's reactive deps in the subscriber graph.
        {
            let mut sub_cx = cx.subscribe_fragment(frag_id);
            kind.subscribe(&mut sub_cx);
        }

        // Insert the empty fragment FIRST so items can auto-link to it.
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

struct RutEachFragment {
    items: AnyReadable,
    builder: EachBuilder,
}

impl FragmentKind for RutEachFragment {
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
        cx.subscribe_readable(self.items);
    }

    fn perform_update(
        &mut self,
        cx: &mut dyn ViewCx,
        fragment_id: FragmentNodeId,
    ) -> Option<Vec<NodeId>> {
        // Rebuild-all reconciliation (the JS Each semantics): the item
        // subtrees are stateless widgets so rebuilding them is cheap. The
        // builder entries run through the guarded face — flush-safe.
        let store = cx.store_read_only();
        let value = store.read(self.items, None);
        let Value::List(items) = &value else {
            return Some(Vec::new());
        };
        let mut out = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            let item_str = item.as_str().unwrap_or_default().to_string();
            if let Some(spec) = self.builder.build(index as u64, &item_str) {
                out.push(spec.build(cx, NodeId::from(fragment_id)));
            }
        }
        Some(out)
    }
}

// ---------------------------------------------------------------------------
// Rows.
// ---------------------------------------------------------------------------

/// Declare the C2 rows on the `tur` decl module.
pub fn decl_rows() -> Vec<(String, Vec<rut_core::types::TypeId>, rut_core::types::TypeId)> {
    use rut_core::types::*;
    vec![
        ("rs_each", vec![TY_U64, TY_STR], TY_OPAQUE),
        (
            "el_lazy_list",
            vec![TY_STR, TY_U64, TY_F64],
            TY_OPAQUE,
        ),
        (
            "el_lazy_grid",
            vec![TY_STR, TY_U64, TY_F64, TY_F64],
            TY_OPAQUE,
        ),
    ]
    .into_iter()
    .map(|(n, p, r)| (n.to_string(), p, r))
    .collect()
}

/// Install the C2 bodies.
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // Each: one child per item of a list atom; the item builder is a named
    // `entry fn(index, item) -> opaque`.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_each", (u64, &str) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, atom: u64, cb: &str| {
        let _ = vm;
        let view = RutEachView {
            items: crate::core::edgy::reactive::Readable::Source(
                crate::core::edgy::reactive::Source::<Value>::from_id(crate::core::edgy::reactive::AtomId(atom as u32)),
            )
            .to_any(),
            builder: EachBuilder {
                name: cb.to_string(),
                face: h.face.clone(),
                handles: h.clone(),
            },
            query_key: Some(vec!["rut".to_string(), "each".to_string()]),
        };
        Ok(rut_vm::Opaque::alloc(vm, RutView(Rc::new(view)))?.handle().clone())
    });

    // LazyList: entry-builder + reactive count + static config. The row
    // shape is (cb, count_atom, item_extent) — axis/overscan default.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_lazy_list", (&str, u64, f64) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, cb: &str, count_atom: u64, item_extent: f64| {
        let _ = vm;
        let entry = RutEntryBuilder {
            name: cb.to_string(),
            face: h.face.clone(),
            handles: h.clone(),
        };
        let view = crate::builtin_plugins::lazy_container::LazyListView::new_rut(
            entry,
            crate::core::view::Val::Reactive(crate::core::edgy::reactive::Readable::Source(
                crate::core::edgy::reactive::Source::<u64>::from_id(crate::core::edgy::reactive::AtomId(count_atom as u32)),
            )),
            Some(crate::core::layout::Axis::Vertical),
            Some(3),
            if item_extent > 0.0 { Some(item_extent) } else { None },
        );
        Ok(rut_vm::Opaque::alloc(vm, RutView(Rc::new(view)))?.handle().clone())
    });

    // LazyGrid: entry-builder + reactive count + max cross extent + aspect.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_lazy_grid", (&str, u64, f64, f64) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, cb: &str, count_atom: u64, max_cross: f64, aspect: f64| {
        let _ = vm;
        let entry = RutEntryBuilder {
            name: cb.to_string(),
            face: h.face.clone(),
            handles: h.clone(),
        };
        let view = crate::builtin_plugins::lazy_container::LazyGridView::new_rut(
            entry,
            crate::core::view::Val::Reactive(crate::core::edgy::reactive::Readable::Source(
                crate::core::edgy::reactive::Source::<u64>::from_id(crate::core::edgy::reactive::AtomId(count_atom as u32)),
            )),
            Some(crate::core::layout::Axis::Vertical),
            Some(3),
            max_cross,
            if aspect > 0.0 { Some(aspect) } else { None },
        );
        Ok(rut_vm::Opaque::alloc(vm, RutView(Rc::new(view)))?.handle().clone())
    });
}
