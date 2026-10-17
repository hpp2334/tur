use std::rc::Rc;

use crate::core::edgy::value::{FromValue, Value};
use crate::core::element::{FragmentNodeId, NodeId};
use crate::core::elements::{FragmentHost, FragmentKind, TraceValue};
use crate::core::layout::SubscribeCx;
use crate::core::view::{Val, View, ViewCx, ViewFactory, read_val};

// ---------------------------------------------------------------------------
// SwitchKey — a raw comparison key. Stores the original value verbatim as a
// native `Value` (opaque keys keep their handle identity) so any value can
// be a case key. Equality is `Value`'s `PartialEq` (primitives by value,
// opaque handles by identity — `same_value_zero` semantics).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct SwitchKey(pub Value);

/// Native-KV decode: the key is held verbatim as a `Value` (opaque keys
/// keep their handle identity; see `edgy::Value`). Comparison rides
/// `Value`'s `PartialEq` (`same_value_zero` semantics — primitives by
/// value, opaque handles by identity), which matches JS `switch`.
impl FromValue for SwitchKey {
    fn from_value(v: &Value) -> Result<Self, String> {
        Ok(SwitchKey(v.clone()))
    }
}

// ---------------------------------------------------------------------------
// SwitchView — the user's declaration.
//
// `value` is reactive (`Val<SwitchKey>`). `cases` is an ordered list of
// (key, branch factory) pairs. SwitchView is a **fragment**: it mounts
// one branch and relays layout to it.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SwitchView {
    value: Val<SwitchKey>,
    cases: Vec<(SwitchKey, Rc<dyn ViewFactory>)>,
    fallback: Option<Rc<dyn ViewFactory>>,
    query_key: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Which branch is currently mounted.
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
pub enum Mounted {
    None,
    Case(SwitchKey),
    Fallback,
}

impl Mounted {
    /// Resolve which branch should be mounted for a given (possibly absent)
    /// current value. Absent value / no matching case → fallback (if any).
    fn resolve(spec: &SwitchView, value: Option<SwitchKey>) -> Mounted {
        match value {
            Some(k) => {
                if spec.cases.iter().any(|(key, _)| *key == k) {
                    Mounted::Case(k)
                } else if spec.fallback.is_some() {
                    Mounted::Fallback
                } else {
                    Mounted::None
                }
            }
            None => {
                if spec.fallback.is_some() {
                    Mounted::Fallback
                } else {
                    Mounted::None
                }
            }
        }
    }

    /// The factory to build for this branch (None for `Mounted::None`).
    fn factory(&self, spec: &SwitchView) -> Option<Rc<dyn ViewFactory>> {
        match self {
            Mounted::Case(k) => spec
                .cases
                .iter()
                .find(|(key, _)| *key == *k)
                .map(|(_, f)| f.clone()),
            Mounted::Fallback => spec.fallback.clone(),
            Mounted::None => None,
        }
    }
}

impl View for SwitchView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let id = cx.alloc_node();
        let frag_id = FragmentNodeId::new(id.as_u64());

        let value = read_val(cx, &self.value);
        let mounted = Mounted::resolve(self, value);

        let kind = SwitchFragment {
            view: self.clone(),
            mounted: mounted.clone(),
        };

        // Register the fragment's reactive deps in the subscriber graph.
        {
            let mut sub_cx = cx.subscribe_fragment(frag_id);
            kind.subscribe(&mut sub_cx);
        }

        // Insert the empty fragment FIRST so the branch can auto-link to it.
        let host = FragmentHost {
            id: frag_id,
            parent,
            children: Vec::new(),
            kind: Some(Box::new(kind)),
            query_key: self.query_key.clone(),
        };
        cx.insert_fragment(host);

        // Build the initial branch — auto-links to the fragment.
        let kind = SwitchFragment {
            view: self.clone(),
            mounted,
        };
        kind.build_branch(cx, frag_id);

        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// SwitchFragment — the `FragmentKind` impl.
// ---------------------------------------------------------------------------

pub struct SwitchFragment {
    view: SwitchView,
    mounted: Mounted,
}

impl SwitchFragment {
    fn build_branch(&self, cx: &mut dyn ViewCx, fragment_id: FragmentNodeId) -> Vec<NodeId> {
        if let Some(view) = self.mounted.factory(&self.view).and_then(|f| f.create()) {
            return vec![view.build(cx, NodeId::from(fragment_id))];
        }
        Vec::new()
    }
}

impl FragmentKind for SwitchFragment {
    fn type_name(&self) -> &'static str {
        "tur_switch"
    }

    fn trace_label(&self, _children: &[NodeId]) -> String {
        match &self.mounted {
            Mounted::None => "branch=none".to_string(),
            Mounted::Case(k) => format!("branch=case({:?})", k),
            Mounted::Fallback => "branch=fallback".to_string(),
        }
    }

    fn trace_props(&self, _children: &[NodeId]) -> Vec<(&'static str, TraceValue)> {
        let branch = match &self.mounted {
            Mounted::None => "none".to_string(),
            Mounted::Case(k) => format!("case({:?})", k),
            Mounted::Fallback => "fallback".to_string(),
        };
        vec![("mountedBranch", TraceValue::Str(branch))]
    }

    fn subscribe(&self, cx: &mut SubscribeCx) {
        cx.subscribe_val(&self.view.value);
    }

    fn perform_update(
        &mut self,
        cx: &mut dyn ViewCx,
        fragment_id: FragmentNodeId,
    ) -> Option<Vec<NodeId>> {
        let new_value = read_val(cx, &self.view.value);
        let new_mounted = Mounted::resolve(&self.view, new_value);
        if new_mounted == self.mounted {
            return None;
        }
        self.mounted = new_mounted;
        Some(self.build_branch(cx, fragment_id))
    }
}

// ---------------------------------------------------------------------------
// Rut-rail constructor (`core::rut_runtime`).
// ---------------------------------------------------------------------------

/// A view factory over an already-built view (the rut rail's crossing —
/// branches are pre-built opaques, no scripting invocation during flush).
pub struct Prebuilt(pub Rc<dyn View>);

impl SwitchView {
    /// Rut builder rows (`core::rut_runtime`): append a pre-built case
    /// branch / install the fallback.
    pub fn push_case(&mut self, key: SwitchKey, branch: Rc<dyn ViewFactory>) {
        self.cases.push((key, branch));
    }
    pub fn set_fallback(&mut self, branch: Rc<dyn ViewFactory>) {
        self.fallback = Some(branch);
    }
    pub(crate) fn set_query_key(&mut self, key: Vec<String>) {
        self.query_key = Some(key);
    }

    /// Re-bind the switch's reactive value (the rut rows' setter twin of
    /// the constructor's `value` prop).
    pub(crate) fn set_value(&mut self, value: Val<SwitchKey>) {
        self.value = value;
    }
}

impl ViewFactory for Prebuilt {
    fn create(&self) -> Option<Rc<dyn View>> {
        Some(self.0.clone())
    }
}

impl SwitchView {
    /// Rut-rail constructor: a switch-key atom with pre-built cases (the
    /// factory clones them — no scripting invocation during flush).
    pub fn new_rut(
        value: Val<SwitchKey>,
        cases: Vec<(SwitchKey, Rc<dyn ViewFactory>)>,
        fallback: Option<Rc<dyn ViewFactory>>,
    ) -> Self {
        SwitchView {
            value,
            cases,
            fallback,
            query_key: None,
        }
    }
}
