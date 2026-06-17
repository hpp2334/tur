use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::widget::{
    extract_spec, val_from_js, Effect, PropValue, Spec, Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// MatchKey — a value-normalized comparison key. One concrete type so `Match`
// stays non-generic while accepting string / number / bool keys from JS.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum MatchKey {
    Num(f64),
    Str(String),
    Bool(bool),
}

impl PropValue for MatchKey {
    fn from_js(v: &boa_engine::JsValue) -> Option<Self> {
        if let Some(b) = v.as_boolean() {
            Some(MatchKey::Bool(b))
        } else if let Some(n) = v.as_number() {
            Some(MatchKey::Num(n))
        } else {
            v.as_string().map(|s| MatchKey::Str(s.to_std_string_escaped()))
        }
    }
}

// ---------------------------------------------------------------------------
// MatchSpec — the user's declaration. Pure Rust, no JsValues.
//
// `value` is reactive (`Val<MatchKey>`). `cases` is an ordered list of
// (key, branch spec) pairs; the first pair whose key equals the current value
// is mounted. `fallback` is mounted when no case matches. Match is a
// transparent widget (like Condition): it relays layout/paint to one branch.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct MatchSpec {
    pub value: Val<MatchKey>,
    pub cases: Vec<(MatchKey, Rc<dyn Spec>)>,
    pub fallback: Option<Rc<dyn Spec>>,
    pub query_key: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Which branch is currently mounted.
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
pub(crate) enum Mounted {
    None,
    Case(MatchKey),
    Fallback,
}

impl Mounted {
    /// Resolve which branch should be mounted for a given (possibly absent)
    /// current value. Absent value / no matching case → fallback (if any).
    fn resolve(spec: &MatchSpec, value: Option<MatchKey>) -> Mounted {
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

    /// The spec to build for this branch (None for `Mounted::None`).
    fn spec(&self, spec: &MatchSpec) -> Option<Rc<dyn Spec>> {
        match self {
            Mounted::Case(k) => spec
                .cases
                .iter()
                .find(|(key, _)| *key == *k)
                .map(|(_, s)| s.clone()),
            Mounted::Fallback => spec.fallback.clone(),
            Mounted::None => None,
        }
    }
}

impl Spec for MatchSpec {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();

        let value = cx.read_val(&self.value, boa);
        let mounted = Mounted::resolve(self, value);

        // Build the chosen branch BEFORE inserting this node so the branch's
        // self-link to `id` is a no-op (parent not yet in the tree). We then
        // explicitly link the branch after inserting — yielding a single edge.
        let mounted_child = self
            .mounted_spec_for(&mounted)
            .map(|spec| spec.build(cx, boa, id));

        cx.insert_node(
            id,
            AnyElement::new(Match {
                spec: self.clone(),
                node_id: id,
                mounted,
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

impl MatchSpec {
    fn mounted_spec_for(&self, mounted: &Mounted) -> Option<Rc<dyn Spec>> {
        mounted.spec(self)
    }
}

// ---------------------------------------------------------------------------
// Match — the built element.
// ---------------------------------------------------------------------------

pub struct Match {
    pub spec: MatchSpec,
    pub(crate) node_id: ElementNodeId,
    pub(crate) mounted: Mounted,
    pub(crate) mounted_child: Option<ElementNodeId>,
}

impl Effect for Match {
    fn effect(
        &mut self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        dirties: &std::collections::HashSet<crate::core::reactive::AtomId>,
    ) {
        if !self.spec.value.is_dirty(dirties) {
            return;
        }

        let new_value = cx.read_val(&self.spec.value, boa);
        let new_mounted = Mounted::resolve(&self.spec, new_value);
        if new_mounted == self.mounted {
            return;
        }

        // Tear down the previously mounted branch.
        if let Some(old) = self.mounted_child.take() {
            cx.destroy_subtree(old);
        }

        // Build the new branch. Match's node IS in the tree during the
        // effect, so the branch's self-link succeeds — no explicit link needed.
        let node_id = self.node_id;
        if let Some(spec) = new_mounted.spec(&self.spec) {
            let child_id = spec.build(cx, boa, node_id);
            self.mounted_child = Some(child_id);
        } else {
            self.mounted_child = None;
        }
        self.mounted = new_mounted;
        cx.mark_dirty(node_id);
    }
}

impl ElementTrace for Match {
    fn trace_label(&self) -> String {
        match &self.mounted {
            Mounted::None => "branch=none".to_string(),
            Mounted::Case(k) => format!("branch=case({:?})", k),
            Mounted::Fallback => "branch=fallback".to_string(),
        }
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

fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn Spec>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    if v.is_undefined() || v.is_null() {
        return None;
    }
    extract_spec(&v)
}

/// Parse `cases` — a JS array of `[key, element]` pairs — into an ordered
/// `Vec<(MatchKey, Spec)>`.
fn prop_cases(props: &JsObject, key: &str, ctx: &mut Context) -> Vec<(MatchKey, Rc<dyn Spec>)> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;

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

    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len as i64 {
        let Ok(entry) = arr.at(i, ctx) else {
            continue;
        };
        let Some(entry_obj) = entry.as_object() else {
            continue;
        };
        let Ok(entry_arr) = JsArray::from_object(entry_obj.clone()) else {
            continue;
        };
        let key_val = entry_arr.at(0, ctx).ok().unwrap_or(boa_engine::JsValue::undefined());
        let spec_val = entry_arr.at(1, ctx).ok().unwrap_or(boa_engine::JsValue::undefined());
        let Some(k) = MatchKey::from_js(&key_val) else {
            continue;
        };
        let Some(spec) = extract_spec(&spec_val) else {
            continue;
        };
        out.push((k, spec));
    }
    out
}

impl MatchSpec {
    /// Build a `MatchSpec` from a JS props object.
    ///
    /// `value` is the reactive key; `cases` is a list of `[key, element]`
    /// pairs; `fallback` is the optional default branch.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        MatchSpec {
            value: prop_val::<MatchKey>(props, "value", ctx)
                .unwrap_or(Val::Static(MatchKey::Str(String::new()))),
            cases: prop_cases(props, "cases", ctx),
            fallback: prop_child(props, "fallback", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
        }
    }
}
