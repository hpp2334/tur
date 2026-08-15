use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::element::{FragmentNodeId, NodeId};
use crate::core::elements::{FragmentHost, FragmentKind, TraceValue};
use crate::core::js_runtime::JsProps;
use crate::core::layout::SubscribeCx;
use crate::core::view::{FromJs, JsViewFactory, Val, View, ViewCx, ViewFactory, read_val};

// ---------------------------------------------------------------------------
// SwitchKey — a raw JS comparison key. Stores the original `JsValue` verbatim
// so any JS value can be a case key. Equality is `JsValue`'s derived
// `PartialEq` (`same_value_zero`), which matches JS `switch` semantics and
// needs no boa `Context`.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct SwitchKey(pub JsValue);

impl FromJs for SwitchKey {
    fn from_js(v: &JsValue) -> Result<Self, boa_engine::JsError> {
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
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id = cx.alloc_node();
        let frag_id = id.as_fragment_id();

        let value = read_val(cx, &self.value, boa);
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
        kind.build_branch(cx, boa, frag_id);

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
    fn build_branch(
        &self,
        cx: &mut dyn ViewCx,
        boa: &mut Context,
        fragment_id: FragmentNodeId,
    ) -> Vec<NodeId> {
        if let Some(factory) = self.mounted.factory(&self.view)
            && let Some(view) = factory.create(boa)
        {
            return vec![view.build(cx, boa, NodeId::from(fragment_id))];
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
        boa: &mut Context,
        fragment_id: FragmentNodeId,
    ) -> Option<Vec<NodeId>> {
        let new_value = read_val(cx, &self.view.value, boa);
        let new_mounted = Mounted::resolve(&self.view, new_value);
        if new_mounted == self.mounted {
            return None;
        }
        self.mounted = new_mounted;
        Some(self.build_branch(cx, boa, fragment_id))
    }
}

// ---------------------------------------------------------------------------
// Factory — parse props into a spec.
// ---------------------------------------------------------------------------

/// Parse `cases` — a JS array of `{ key, child }` entries.
fn prop_cases(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Vec<(SwitchKey, Rc<dyn ViewFactory>)> {
    use boa_engine::js_string;
    use boa_engine::object::builtins::{JsArray, JsFunction};

    let v = match props.get(js_string!(key), ctx) {
        Ok(v) if !v.is_undefined() && !v.is_null() => v,
        _ => return Vec::new(),
    };
    let Some(obj) = v.as_object() else {
        return Vec::new();
    };
    let Ok(arr) = JsArray::from_object(obj.clone()) else {
        return Vec::new();
    };
    let Ok(len) = arr.length(ctx) else {
        return Vec::new();
    };

    let mut out: Vec<(SwitchKey, Rc<dyn ViewFactory>)> = Vec::with_capacity(len as usize);
    for i in 0..len as i64 {
        let Ok(entry) = arr.at(i, ctx) else {
            continue;
        };
        let Some(entry_obj) = entry.as_object() else {
            continue;
        };
        let key_val = entry_obj
            .get(js_string!("key"), ctx)
            .unwrap_or(JsValue::undefined());
        let child_val = entry_obj
            .get(js_string!("child"), ctx)
            .unwrap_or(JsValue::undefined());
        let Some(f) = child_val.as_object().and_then(JsFunction::from_object) else {
            continue;
        };
        out.push((SwitchKey(key_val), Rc::new(JsViewFactory(f))));
    }
    out
}

impl SwitchView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let (value, fallback, query_key) = {
            let mut p = JsProps::new(props, ctx);
            (
                p.val::<SwitchKey>("value")
                    .unwrap_or_else(|| Val::Static(SwitchKey(JsValue::undefined()))),
                p.factory("fallback"),
                p.query_key("queryKey"),
            )
        };
        SwitchView {
            value,
            cases: prop_cases(props, "cases", ctx),
            fallback,
            query_key,
        }
    }
}
