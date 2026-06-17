use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::reactive::{extract_atom, AtomId};
use crate::core::widget::{extract_spec, Effect, Spec, WidgetCx};

// ---------------------------------------------------------------------------
// DynamicChild — either a static spec or a reactive atom whose value is itself
// an EdgyElement (a spec handle). `Rc<dyn Spec>` is resolved via
// `extract_spec` (not `PropValue::from_js`), so it can't ride on `Val<T>`;
// this enum mirrors `val_from_js` for spec handles.
// ---------------------------------------------------------------------------

pub enum DynamicChild {
    Static(Rc<dyn Spec>),
    Reactive(AtomId),
}

impl DynamicChild {
    /// Interpret a JS prop as a dynamic child: an atom handle → `Reactive`;
    /// otherwise a spec handle → `Static`. Returns `None` for absent values
    /// or values that are neither an atom nor a spec.
    fn from_js(v: &boa_engine::JsValue) -> Option<Self> {
        if v.is_undefined() || v.is_null() {
            return None;
        }
        match extract_atom(v) {
            Some(id) => Some(DynamicChild::Reactive(id)),
            None => extract_spec(v).map(DynamicChild::Static),
        }
    }
}

// ---------------------------------------------------------------------------
// DynamicSpec — render an element produced reactively at runtime. When the
// atom yields a different spec object, the subtree is rebuilt. Mirrors
// Flutter's `ValueListenableBuilder` (rebuild subtree when a listenable
// changes).
// ---------------------------------------------------------------------------

pub struct DynamicSpec {
    pub child: DynamicChild,
    pub query_key: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Dynamic — the built element. Holds its spec, its node id, the currently
// mounted spec (for identity comparison), and the mounted child id.
// ---------------------------------------------------------------------------

pub struct Dynamic {
    pub(crate) node_id: ElementNodeId,
    pub(crate) child: DynamicChild,
    pub(crate) mounted_spec: Option<Rc<dyn Spec>>,
    pub(crate) mounted_child: Option<ElementNodeId>,
}

/// Read the atom's current value as a spec handle (untracked). Returns `None`
/// when the atom holds a non-spec value (e.g. undefined) — treated as "no
/// child".
fn resolve_reactive_spec(cx: &WidgetCx, atom: AtomId, boa: &mut Context) -> Option<Rc<dyn Spec>> {
    let raw = cx.read_atom_raw(atom, boa);
    extract_spec(&raw)
}

/// Identity comparison for spec handles. Stable across reads of an unchanged
/// atom because boa returns the same `SpecHandle` object, so `extract_spec`
/// yields the same `Rc` allocation.
fn spec_ptr_eq(a: &Option<Rc<dyn Spec>>, b: &Option<Rc<dyn Spec>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

impl Spec for DynamicSpec {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();

        let (mounted_spec, mounted_child) = match &self.child {
            DynamicChild::Static(spec) => {
                let child_id = spec.build(cx, boa, id);
                (Some(spec.clone()), Some(child_id))
            }
            DynamicChild::Reactive(atom) => {
                let spec = resolve_reactive_spec(cx, *atom, boa);
                let child_id = spec.as_ref().map(|s| s.build(cx, boa, id));
                (spec, child_id)
            }
        };

        cx.insert_node(
            id,
            AnyElement::new(Dynamic {
                node_id: id,
                child: clone_child(&self.child),
                mounted_spec,
                mounted_child,
            }),
            boa,
        );

        if let Some(child_id) = mounted_child {
            cx.link_child(id, child_id);
        }
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        cx.link_child(parent, id);
        id
    }
}

fn clone_child(c: &DynamicChild) -> DynamicChild {
    match c {
        DynamicChild::Static(s) => DynamicChild::Static(s.clone()),
        DynamicChild::Reactive(id) => DynamicChild::Reactive(*id),
    }
}

impl Effect for Dynamic {
    fn effect(
        &mut self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        dirties: &std::collections::HashSet<AtomId>,
    ) {
        let atom = match &self.child {
            DynamicChild::Reactive(atom) => *atom,
            // Static children never change — nothing to do.
            DynamicChild::Static(_) => return,
        };
        if !dirties.contains(&atom) {
            return;
        }

        let new_spec = resolve_reactive_spec(cx, atom, boa);
        if spec_ptr_eq(&new_spec, &self.mounted_spec) {
            return;
        }

        // Tear down the previously mounted subtree.
        if let Some(old) = self.mounted_child.take() {
            cx.destroy_subtree(old);
        }

        // Build the new subtree. Dynamic's node IS in the tree during the
        // effect, so the branch's self-link succeeds.
        let node_id = self.node_id;
        if let Some(spec) = new_spec.as_ref() {
            let child_id = spec.build(cx, boa, node_id);
            self.mounted_child = Some(child_id);
        } else {
            self.mounted_child = None;
        }
        self.mounted_spec = new_spec;
        cx.mark_dirty(node_id);
    }
}

impl ElementTrace for Dynamic {
    fn trace_label(&self) -> String {
        match &self.mounted_spec {
            Some(_) => "mounted".to_string(),
            None => "empty".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Factory — parse props into a spec.
// ---------------------------------------------------------------------------

fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<DynamicChild> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    DynamicChild::from_js(&v)
}

fn prop_query_key(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Vec<String>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    let obj = v.as_object()?;
    let arr = JsArray::from_object(obj.clone()).ok()?;
    let len = arr.length(ctx).ok()? as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if let Ok(val) = arr.at(i as i64, ctx) {
            if let Some(s) = val.as_string() {
                out.push(s.to_std_string_escaped());
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

impl DynamicSpec {
    /// Build a `DynamicSpec` from a JS props object.
    ///
    /// `child` is either an EdgyElement (static) or an atom producing one
    /// (reactive).
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        DynamicSpec {
            child: prop_child(props, "child", ctx).unwrap_or(DynamicChild::Static(Rc::new(
                crate::elements::FragmentSpec {
                    children: Vec::new(),
                    query_key: None,
                },
            ))),
            query_key: prop_query_key(props, "queryKey", ctx),
        }
    }
}
