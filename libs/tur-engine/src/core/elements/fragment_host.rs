use std::collections::HashSet;

use boa_engine::Context;

use crate::core::element::{FragmentNodeId, NodeId};
use crate::core::elements::TraceValue;
use crate::core::reactive::AtomId;
use crate::core::widget::WidgetCx;

/// A **control-flow primitive** (Each / Condition / Switch) that lives in the
/// tree as a non-element node. Fragments own a `children` list but contribute
/// **no layout box**: the enclosing flex lays the fragment's children out
/// directly as its own items (via `flatten_children`), so an `Each` inside a
/// `Row` flows horizontally and an `Each` inside a `Column` flows vertically —
/// inheriting the parent's axis and sizing.
///
/// Fragments are **not** stored in `ElementTree::nodes` (they have no
/// `AnyElement`, no layout/render/subscribe). They live in the separate
/// `fragments` map and are referenced from real elements' `children` as plain
/// `NodeId`s (distinguished from real elements via `ElementTree::is_fragment`).
///
/// Reactivity: during each reactive flush, `run_effects` takes the `kind` out,
/// calls `try_rebuild`, and if the branch/items changed, swaps `children` and
/// marks the **real** ancestor dirty so the parent flex re-lays-out with the
/// new flattened children in the same flush iteration.
pub struct FragmentHost {
    pub id: FragmentNodeId,
    /// The nearest **real** element ancestor — used by `mark_dirty` so the
    /// parent flex is re-laid-out when this fragment's children change.
    pub parent: NodeId,
    /// Currently-mounted children (can be nested fragments).
    pub children: Vec<NodeId>,
    /// The primitive-specific state + rebuild logic. `Option` so it can be
    /// taken out during the effect pass (mirrors `node.element.take()`),
    /// letting the host stay in the tree so children can auto-link.
    pub kind: Option<Box<dyn FragmentKind>>,
    pub query_key: Option<Vec<String>>,
}

impl FragmentHost {
    pub fn type_name(&self) -> &'static str {
        self.kind.as_ref().map(|k| k.type_name()).unwrap_or("tur_fragment")
    }

    pub fn trace_label(&self) -> String {
        match self.kind.as_ref() {
            Some(k) => k.trace_label(&self.children),
            None => format!("items={}", self.children.len()),
        }
    }

    pub fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        match self.kind.as_ref() {
            Some(k) => k.trace_props(&self.children),
            None => vec![],
        }
    }
}

/// The primitive-specific rebuild logic. Each control-flow primitive
/// (Condition / Each / Switch) implements this with its reactive prop and
/// branch/item resolution.
pub trait FragmentKind: 'static {
    fn type_name(&self) -> &'static str;

    /// Human-readable label for the dev tool (uses current children).
    fn trace_label(&self, children: &[NodeId]) -> String;

    /// Structured props for the dev tool (uses current children).
    fn trace_props(&self, children: &[NodeId]) -> Vec<(&'static str, TraceValue)>;

    /// Check if the reactive prop is dirty and, if the resolved branch/items
    /// changed, return `Some(new_children)` (built under `fragment_id`). Return
    /// `None` if no rebuild is needed.
    ///
    /// The returned children are built via `Component::build(cx, boa,
    /// fragment_id)` — each child auto-links itself to the fragment (pushing
    /// to `fragments[fragment_id].children`).
    fn try_rebuild(
        &mut self,
        cx: &mut WidgetCx,
        boa: &mut Context,
        dirties: &HashSet<AtomId>,
        fragment_id: FragmentNodeId,
    ) -> Option<Vec<NodeId>>;
}
