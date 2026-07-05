use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::element::{FragmentNodeId, NodeId};
use crate::core::elements::{FragmentHost, FragmentKind, TraceValue};
use crate::core::layout::SubscribeCx;
use crate::core::view::{
    ViewCx,
    read_val,
    val_from_js, View, ViewFactory, JsViewFactory, PropValue, Val,
};

// ---------------------------------------------------------------------------
// SwitchKey — a raw JS comparison key. Stores the original `JsValue` verbatim
// so any JS value can be a case key. Equality is `JsValue`'s derived
// `PartialEq` (`same_value_zero`), which matches JS `switch` semantics and
// needs no boa `Context`.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct SwitchKey(pub JsValue);

impl PropValue for SwitchKey {
    fn from_js(v: &JsValue) -> Option<Self> {
        Some(SwitchKey(v.clone()))
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
    pub value: Val<SwitchKey>,
    pub cases: Vec<(SwitchKey, Rc<dyn ViewFactory>)>,
    pub fallback: Option<Rc<dyn ViewFactory>>,
    pub query_key: Option<Vec<String>>,
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
        let frag_id = FragmentNodeId::new(id.as_u64());

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
    pub view: SwitchView,
    pub mounted: Mounted,
}

impl SwitchFragment {
    fn build_branch(
        &self,
        cx: &mut dyn ViewCx,
        boa: &mut Context,
        fragment_id: FragmentNodeId,
    ) -> Vec<NodeId> {
        if let Some(factory) = self.mounted.factory(&self.view) {
            if let Some(view) = factory.create(boa) {
                return vec![view.build(cx, boa, NodeId::from(fragment_id))];
            }
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

fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
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

/// Extract an optional branch factory from a JS props object.
fn prop_factory(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn ViewFactory>> {
    use boa_engine::js_string;
    use boa_engine::object::builtins::JsFunction;
    let v = props.get(js_string!(key), ctx).ok()?;
    if v.is_undefined() || v.is_null() {
        return None;
    }
    let f = v.as_object().and_then(JsFunction::from_object)?;
    Some(Rc::new(JsViewFactory(f)))
}

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
        SwitchView {
            value: prop_val::<SwitchKey>(props, "value", ctx)
                .unwrap_or_else(|| Val::Static(SwitchKey(JsValue::undefined()))),
            cases: prop_cases(props, "cases", ctx),
            fallback: prop_factory(props, "fallback", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
        }
    }
}
