use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::element::{FragmentNodeId, NodeId};
use crate::core::elements::{FragmentHost, FragmentKind, TraceValue};
use crate::core::widget::{
    val_from_js, Component, ComponentFactory, JsComponentFactory, PropValue, Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// ConditionComponent — the user's declaration. Pure Rust, no JsValues.
//
// `condition` is reactive (`Val<bool>`). `then_child` / `else_child` are the
// branch factories (`Option<ComponentFactory>`): the concrete subtree is only
// known at runtime, so `create()` is invoked when a branch is selected.
//
// ConditionComponent is a **fragment**: it mounts exactly one branch and has no
// layout box of its own — the enclosing flex lays the branch out directly.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ConditionComponent {
    pub condition: Val<bool>,
    pub then_child: Option<Rc<dyn ComponentFactory>>,
    pub else_child: Option<Rc<dyn ComponentFactory>>,
    pub query_key: Option<Vec<String>>,
}

impl Component for ConditionComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id = cx.alloc_node();

        // Resolve the initial condition value and pick the branch.
        let value = cx.read_val(&self.condition, boa).unwrap_or(false);
        let mounted = if value {
            MountedBranch::Then
        } else {
            MountedBranch::Else
        };

        // Insert the empty fragment FIRST so the branch can auto-link to it.
        let host = FragmentHost {
            id: FragmentNodeId::new(id.as_u64()),
            parent,
            children: Vec::new(),
            kind: Some(Box::new(ConditionFragment {
                component: self.clone(),
                mounted,
            })),
            query_key: self.query_key.clone(),
        };
        cx.insert_fragment(host);

        // Build the initial branch — auto-links to the fragment.
        let kind = ConditionFragment {
            component: self.clone(),
            mounted,
        };
        kind.build_branch(cx, boa, FragmentNodeId::new(id.as_u64()));

        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// Which branch is currently mounted.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountedBranch {
    Then,
    Else,
    None,
}

// ---------------------------------------------------------------------------
// ConditionFragment — the `FragmentKind` impl. Holds the spec + which branch
// is mounted. `try_rebuild` checks if `condition` is dirty and swaps branches.
// ---------------------------------------------------------------------------

pub struct ConditionFragment {
    pub component: ConditionComponent,
    pub mounted: MountedBranch,
}

impl ConditionFragment {
    fn current_factory(&self) -> Option<Rc<dyn ComponentFactory>> {
        match self.mounted {
            MountedBranch::Then => self.component.then_child.clone(),
            MountedBranch::Else => self.component.else_child.clone(),
            MountedBranch::None => None,
        }
    }

    /// Build the currently-mounted branch under `fragment_id`.
    fn build_branch(
        &self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        fragment_id: FragmentNodeId,
    ) -> Vec<NodeId> {
        if let Some(factory) = self.current_factory() {
            if let Some(component) = factory.create(boa) {
                return vec![component.build(cx, boa, NodeId::from(fragment_id))];
            }
        }
        Vec::new()
    }
}

impl FragmentKind for ConditionFragment {
    fn type_name(&self) -> &'static str {
        "tur_condition"
    }

    fn trace_label(&self, _children: &[NodeId]) -> String {
        let branch = match self.mounted {
            MountedBranch::Then => "then",
            MountedBranch::Else => "else",
            MountedBranch::None => "none",
        };
        format!("branch={branch}")
    }

    fn trace_props(&self, _children: &[NodeId]) -> Vec<(&'static str, TraceValue)> {
        let branch = match self.mounted {
            MountedBranch::Then => "then",
            MountedBranch::Else => "else",
            MountedBranch::None => "none",
        };
        vec![("mountedBranch", TraceValue::Str(branch.to_string()))]
    }

    fn try_rebuild(
        &mut self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        dirties: &std::collections::HashSet<crate::core::reactive::AtomId>,
        fragment_id: FragmentNodeId,
    ) -> Option<Vec<NodeId>> {
        if !self.component.condition.is_dirty(dirties) {
            return None;
        }

        let new_value = cx.read_val(&self.component.condition, boa).unwrap_or(false);
        let new_branch = if new_value {
            MountedBranch::Then
        } else {
            MountedBranch::Else
        };
        if new_branch == self.mounted {
            return None;
        }
        self.mounted = new_branch;
        Some(self.build_branch(cx, boa, fragment_id))
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
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
    if out.is_empty() { None } else { Some(out) }
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

impl ConditionComponent {
    /// Build a `ConditionComponent` from a JS props object.
    ///
    /// `child` is the then-branch, `elseChild` is the else-branch (mirroring
    /// the JS `ConditionProps` interface). Both are thunks `() => EdgyElement`.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        ConditionComponent {
            condition: prop_val::<bool>(props, "condition", ctx)
                .unwrap_or(Val::Static(false)),
            then_child: prop_factory(props, "child", ctx),
            else_child: prop_factory(props, "elseChild", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
        }
    }
}
