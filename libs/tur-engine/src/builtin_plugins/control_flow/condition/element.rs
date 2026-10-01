use std::rc::Rc;

use crate::core::element::{FragmentNodeId, NodeId};
use crate::core::elements::{FragmentHost, FragmentKind, TraceValue};
use crate::core::layout::SubscribeCx;
use crate::core::view::{Val, View, ViewCx, ViewFactory, read_val};

// ---------------------------------------------------------------------------
// ConditionView — the user's declaration. Pure Rust, no JsValues.
//
// `condition` is reactive (`Val<bool>`). `then_child` / `else_child` are the
// branch factories (`Option<ViewFactory>`): the concrete subtree is only
// known at runtime, so `create()` is invoked when a branch is selected.
//
// ConditionView is a **fragment**: it mounts exactly one branch and has no
// layout box of its own — the enclosing flex lays the branch out directly.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ConditionView {
    condition: Val<bool>,
    then_child: Option<Rc<dyn ViewFactory>>,
    else_child: Option<Rc<dyn ViewFactory>>,
    query_key: Option<Vec<String>>,
}

impl View for ConditionView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let id = cx.alloc_node();
        let frag_id = FragmentNodeId::new(id.as_u64());

        // Resolve the initial condition value and pick the branch.
        let value = read_val(cx, &self.condition).unwrap_or(false);
        let mounted = if value {
            MountedBranch::Then
        } else {
            MountedBranch::Else
        };

        let kind = ConditionFragment {
            view: self.clone(),
            mounted,
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
        let kind = ConditionFragment {
            view: self.clone(),
            mounted,
        };
        kind.build_branch(cx, frag_id);

        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// Which branch is currently mounted.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
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
    view: ConditionView,
    mounted: MountedBranch,
}

impl ConditionFragment {
    fn current_factory(&self) -> Option<Rc<dyn ViewFactory>> {
        match self.mounted {
            MountedBranch::Then => self.view.then_child.clone(),
            MountedBranch::Else => self.view.else_child.clone(),
            MountedBranch::None => None,
        }
    }

    /// Build the currently-mounted branch under `fragment_id`.
    fn build_branch(&self, cx: &mut dyn ViewCx, fragment_id: FragmentNodeId) -> Vec<NodeId> {
        if let Some(view) = self.current_factory().and_then(|f| f.create()) {
            return vec![view.build(cx, NodeId::from(fragment_id))];
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

    fn subscribe(&self, cx: &mut SubscribeCx) {
        cx.subscribe_val(&self.view.condition);
    }

    fn perform_update(
        &mut self,
        cx: &mut dyn ViewCx,
        fragment_id: FragmentNodeId,
    ) -> Option<Vec<NodeId>> {
        let new_value = read_val(cx, &self.view.condition).unwrap_or(false);
        let new_branch = if new_value {
            MountedBranch::Then
        } else {
            MountedBranch::Else
        };
        if new_branch == self.mounted {
            return None;
        }
        self.mounted = new_branch;
        Some(self.build_branch(cx, fragment_id))
    }
}

// ---------------------------------------------------------------------------
// Rut-rail constructor (`core::rut_runtime`).
// ---------------------------------------------------------------------------

impl ConditionView {
    /// Rut-rail constructor: a bool-atom condition with both branches
    /// pre-built (the factory clones them — no scripting invocation during
    /// flush).
    pub(crate) fn new_rut(
        condition: Val<bool>,
        then_child: Rc<dyn ViewFactory>,
        else_child: Rc<dyn ViewFactory>,
    ) -> Self {
        ConditionView {
            condition,
            then_child: Some(then_child),
            else_child: Some(else_child),
            query_key: None,
        }
    }
}
