use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::widget::{
    val_from_js, Component, ComponentFactory, Effect, JsComponentFactory, PropValue, Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// ConditionComponent — the user's declaration. Pure Rust, no JsValues.
//
// `condition` is reactive (`Val<bool>`). `then_child` / `else_child` are the
// branch factories (`Option<ComponentFactory>`): the concrete subtree is only
// known at runtime, so `create()` is invoked when a branch is selected.
// ConditionElement is a transparent widget: it mounts exactly one branch and relays
// layout/paint to it.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ConditionComponent {
    pub condition: Val<bool>,
    pub then_child: Option<Rc<dyn ComponentFactory>>,
    pub else_child: Option<Rc<dyn ComponentFactory>>,
    pub query_key: Option<Vec<String>>,
}

impl Component for ConditionComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();

        // Resolve the initial condition value and pick the branch.
        let value = cx.read_val(&self.condition, boa).unwrap_or(false);
        let branch = if value {
            MountedBranch::Then
        } else {
            MountedBranch::Else
        };
        let branch_factory = match branch {
            MountedBranch::Then => self.then_child.clone(),
            MountedBranch::Else => self.else_child.clone(),
            MountedBranch::None => None,
        };

        // Build the chosen branch BEFORE inserting this node so the branch's
        // self-link to `id` is a no-op (parent not yet in the tree). We then
        // explicitly link the branch after inserting — yielding a single edge.
        let (mounted, mounted_child) = if let Some(factory) = branch_factory {
            if let Some(component) = factory.create(boa) {
                let child_id = component.build(cx, boa, id);
                (branch, Some(child_id))
            } else {
                (MountedBranch::None, None)
            }
        } else {
            (MountedBranch::None, None)
        };

        cx.insert_node(
            id,
            AnyElement::new(ConditionElement {
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
// ConditionElement — the built element. Holds its spec, its own node id (so the
// effect can rebuild branches), and which branch is currently mounted.
// ---------------------------------------------------------------------------

pub struct ConditionElement {
    pub component: ConditionComponent,
    pub(crate) node_id: ElementNodeId,
    pub(crate) mounted: MountedBranch,
    pub(crate) mounted_child: Option<ElementNodeId>,
}

impl Effect for ConditionElement {
    fn effect(
        &mut self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        dirties: &std::collections::HashSet<crate::core::reactive::AtomId>,
    ) {
        if !self.component.condition.is_dirty(dirties) {
            return;
        }

        let new_value = cx.read_val(&self.component.condition, boa).unwrap_or(false);
        let new_branch = if new_value {
            MountedBranch::Then
        } else {
            MountedBranch::Else
        };
        if new_branch == self.mounted {
            return;
        }

        // Tear down the previously mounted branch.
        if let Some(old) = self.mounted_child.take() {
            cx.destroy_subtree(old);
        }

        // Build the new branch. ConditionElement's node IS in the tree during the
        // effect, so the branch's self-link succeeds — no explicit link needed.
        let new_factory = match new_branch {
            MountedBranch::Then => self.component.then_child.clone(),
            MountedBranch::Else => self.component.else_child.clone(),
            MountedBranch::None => None,
        };
        let node_id = self.node_id;
        if let Some(factory) = new_factory {
            if let Some(component) = factory.create(boa) {
                let child_id = component.build(cx, boa, node_id);
                self.mounted_child = Some(child_id);
            } else {
                self.mounted_child = None;
            }
        } else {
            self.mounted_child = None;
        }
        self.mounted = new_branch;
        cx.mark_dirty(node_id);
    }
}

impl ElementTrace for ConditionElement {
    fn trace_label(&self) -> String {
        let branch = match self.mounted {
            MountedBranch::Then => "then",
            MountedBranch::Else => "else",
            MountedBranch::None => "none",
        };
        format!("branch={branch}")
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let branch = match self.mounted {
            MountedBranch::Then => "then",
            MountedBranch::Else => "else",
            MountedBranch::None => "none",
        };
        vec![("mountedBranch", TraceValue::Str(branch.to_string()))]
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
