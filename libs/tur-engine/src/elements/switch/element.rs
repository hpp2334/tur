use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::widget::{
    val_from_js, Component, ComponentFactory, Effect, JsComponentFactory, PropValue, Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// SwitchKey — a raw JS comparison key. It stores the original `JsValue`
// verbatim (no normalization) so any JS value — string, number, boolean,
// null, undefined, bigint, even objects — can be a case key. Equality is
// `JsValue`'s derived `PartialEq` (`same_value_zero`: `===` for primitives,
// pointer identity for objects), which matches JS `switch` semantics and,
// crucially, needs no boa `Context` — so `SwitchElement` can resolve branches during
// layout/paint without touching the JS runtime.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct SwitchKey(pub JsValue);

impl PropValue for SwitchKey {
    fn from_js(v: &JsValue) -> Option<Self> {
        Some(SwitchKey(v.clone()))
    }
}

// ---------------------------------------------------------------------------
// SwitchComponent — the user's declaration.
//
// `value` is reactive (`Val<SwitchKey>`). `cases` is an ordered list of
// (key, branch factory) pairs; the first pair whose key equals the current
// value is mounted. `fallback` is mounted when no case matches. Branches are
// `ComponentFactory`s because the concrete subtree is only known at runtime.
// SwitchElement is a transparent widget (like ConditionElement): it relays layout/paint to
// one branch.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SwitchComponent {
    pub value: Val<SwitchKey>,
    pub cases: Vec<(SwitchKey, Rc<dyn ComponentFactory>)>,
    pub fallback: Option<Rc<dyn ComponentFactory>>,
    pub query_key: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Which branch is currently mounted.
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
pub(crate) enum Mounted {
    None,
    Case(SwitchKey),
    Fallback,
}

impl Mounted {
    /// Resolve which branch should be mounted for a given (possibly absent)
    /// current value. Absent value / no matching case → fallback (if any).
    fn resolve(spec: &SwitchComponent, value: Option<SwitchKey>) -> Mounted {
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
    fn factory(&self, spec: &SwitchComponent) -> Option<Rc<dyn ComponentFactory>> {
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

impl Component for SwitchComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();

        let value = cx.read_val(&self.value, boa);
        let mounted = Mounted::resolve(self, value);

        // Build the chosen branch BEFORE inserting this node so the branch's
        // self-link to `id` is a no-op (parent not yet in the tree). We then
        // explicitly link the branch after inserting — yielding a single edge.
        let mounted_child = self
            .mounted_factory_for(&mounted)
            .and_then(|f| f.create(boa))
            .map(|component| component.build(cx, boa, id));

        cx.insert_node(
            id,
            AnyElement::new(SwitchElement {
                component: self.clone(),
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

impl SwitchComponent {
    fn mounted_factory_for(&self, mounted: &Mounted) -> Option<Rc<dyn ComponentFactory>> {
        mounted.factory(self)
    }
}

// ---------------------------------------------------------------------------
// SwitchElement — the built element.
// ---------------------------------------------------------------------------

pub struct SwitchElement {
    pub component: SwitchComponent,
    pub(crate) node_id: ElementNodeId,
    pub(crate) mounted: Mounted,
    pub(crate) mounted_child: Option<ElementNodeId>,
}

impl Effect for SwitchElement {
    fn effect(
        &mut self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        dirties: &std::collections::HashSet<crate::core::reactive::AtomId>,
    ) {
        if !self.component.value.is_dirty(dirties) {
            return;
        }

        let new_value = cx.read_val(&self.component.value, boa);
        let new_mounted = Mounted::resolve(&self.component, new_value);
        if new_mounted == self.mounted {
            return;
        }

        // Tear down the previously mounted branch.
        if let Some(old) = self.mounted_child.take() {
            cx.destroy_subtree(old);
        }

        // Build the new branch. SwitchElement's node IS in the tree during the
        // effect, so the branch's self-link succeeds — no explicit link needed.
        let node_id = self.node_id;
        if let Some(factory) = new_mounted.factory(&self.component) {
            if let Some(component) = factory.create(boa) {
                let child_id = component.build(cx, boa, node_id);
                self.mounted_child = Some(child_id);
            } else {
                self.mounted_child = None;
            }
        } else {
            self.mounted_child = None;
        }
        self.mounted = new_mounted;
        cx.mark_dirty(node_id);
    }
}

impl ElementTrace for SwitchElement {
    fn trace_label(&self) -> String {
        match &self.mounted {
            Mounted::None => "branch=none".to_string(),
            Mounted::Case(k) => format!("branch=case({:?})", k),
            Mounted::Fallback => "branch=fallback".to_string(),
        }
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let branch = match &self.mounted {
            Mounted::None => "none".to_string(),
            Mounted::Case(k) => format!("case({:?})", k),
            Mounted::Fallback => "fallback".to_string(),
        };
        vec![("mountedBranch", TraceValue::Str(branch))]
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

/// Extract an optional branch factory from a JS props object. The prop value
/// is a JS thunk `() => EdgyElement`; it is wrapped in a `JsComponentFactory`.
fn prop_factory(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn ComponentFactory>> {
    use boa_engine::js_string;
    use boa_engine::object::builtins::JsFunction;
    let v = props.get(js_string!(key), ctx).ok()?;
    if v.is_undefined() || v.is_null() {
        return None;
    }
    let f = v.as_object().and_then(JsFunction::from_object)?;
    Some(Rc::new(JsComponentFactory(f)))
}

/// Parse `cases` — a JS array of `{ key, child }` entries — into an ordered
/// `Vec<(SwitchKey, ComponentFactory)>`. Any `key` value is accepted (stored as
/// a raw `JsValue`); each `child` is a JS thunk `() => EdgyElement` wrapped in a
/// `JsComponentFactory`. An entry is skipped only if its `child` is not callable.
fn prop_cases(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Vec<(SwitchKey, Rc<dyn ComponentFactory>)> {
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

    let mut out: Vec<(SwitchKey, Rc<dyn ComponentFactory>)> = Vec::with_capacity(len as usize);
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
        out.push((SwitchKey(key_val), Rc::new(JsComponentFactory(f))));
    }
    out
}

impl SwitchComponent {
    /// `value` is the reactive key; `cases` is a list of `{ key, child }`
    /// entries (each `child` is a thunk `() => EdgyElement`); `fallback` is the
    /// optional default branch thunk.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        SwitchComponent {
            value: prop_val::<SwitchKey>(props, "value", ctx)
                .unwrap_or_else(|| Val::Static(SwitchKey(JsValue::undefined()))),
            cases: prop_cases(props, "cases", ctx),
            fallback: prop_factory(props, "fallback", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
        }
    }
}
