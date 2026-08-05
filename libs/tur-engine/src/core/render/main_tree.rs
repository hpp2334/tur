//! Long-lived render tree on main, updated by applying batches of
//! [`RenderCommand`]s (commit-log style).
//!
//! The worker-side record pass produces `Vec<RenderCommand>` per frame, which
//! the renderer plays linearly into the scene (see
//! [`crate::core::render::play_commands`]). `MainTree` mirrors the same batch
//! into a persisted topology+paint-state map for non-render queries —
//! dev-tool tree dumps, future main-side hit-testing, and the cursor
//! "last applied" dedup state. The render path itself does NOT consult
//! `MainTree` (the command batch is self-describing: each
//! [`RenderCommand::Paint`] carries its own absolute transform).
//!
//! Topology is diffed: [`build_topology_batch`] emits
//! [`RenderCommand::SetChildren`] only when a node's child list changed and
//! [`RenderCommand::Remove`] for any id that disappears from the worker's
//! tree between frames — steady-state frames ship zero topology commands.

use std::collections::HashMap;
use std::collections::HashSet;

use crate::core::element::ElementNodeId;
use crate::core::layout::Size;
use crate::core::platform::Cursor;
use crate::core::render::CanvasOp;
use crate::core::render::command::RenderCommand;
use vello_common::kurbo::Affine;

/// One node's last-applied paint + topology state.
///
/// All fields are replaced on each [`RenderCommand::Paint`] / `SetChildren`
/// for the id (diffed topology — `SetChildren` only arrives on change).
#[derive(Debug, Clone)]
struct MainNode {
    /// Last `Paint.transform` (absolute affine).
    transform: Affine,
    /// Last `Paint.size`.
    size: Size,
    /// Last `Paint.ops` (replaces previous).
    ops: Vec<CanvasOp>,
    /// Last `SetChildren.child_ids` (flattened, dev-tool queries only).
    child_ids: Vec<ElementNodeId>,
}

impl Default for MainNode {
    fn default() -> Self {
        MainNode {
            transform: Affine::IDENTITY,
            size: Size::ZERO,
            ops: Vec::new(),
            child_ids: Vec::new(),
        }
    }
}

/// The main-side render tree mirror.
///
/// Owned by `MainBackend` on the main thread (the worker→main wire contract
/// is `Vec<RenderCommand>`, applied verbatim via [`MainTree::apply_batch`]).
#[derive(Debug, Default)]
pub struct MainTree {
    nodes: HashMap<ElementNodeId, MainNode>,
    /// First `Paint` id seen — used as a fallback root marker when no
    /// explicit topology has arrived yet. Real topology comes via
    /// `SetChildren` from the worker's full sync.
    root: Option<ElementNodeId>,
    /// Last cursor claim emitted by the worker (`Cursor` command). Used as
    /// the dedup baseline on the worker side. None until the first claim.
    last_cursor: Option<Cursor>,
}

impl MainTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a batch of [`RenderCommand`]s atomically.
    ///
    /// `Paint` / `SetChildren` / `Remove` mutate the topology/paint-state
    /// map; `Cursor` updates the cursor side-channel. The render path
    /// (which iterates the batch itself via `play_commands`) does not
    /// consult `MainTree` — this method only maintains the persisted
    /// query-able state.
    pub fn apply_batch(&mut self, batch: &[RenderCommand]) {
        for cmd in batch {
            match cmd {
                RenderCommand::Paint {
                    id,
                    transform,
                    size,
                    ops,
                } => {
                    let node = self.nodes.entry(*id).or_default();
                    node.transform = *transform;
                    node.size = *size;
                    node.ops.clone_from(ops);
                    if self.root.is_none() {
                        self.root = Some(*id);
                    }
                }
                RenderCommand::SetChildren { id, child_ids } => {
                    let node = self.nodes.entry(*id).or_default();
                    node.child_ids.clone_from(child_ids);
                    if self.root.is_none() {
                        self.root = Some(*id);
                    }
                }
                RenderCommand::Cursor { cursor } => {
                    self.last_cursor = Some(*cursor);
                }
                RenderCommand::Remove { id } => {
                    self.nodes.remove(id);
                    if self.root == Some(*id) {
                        self.root = None;
                    }
                }
            }
        }
    }

    /// First id seen (fallback root marker). Real topology comes via
    /// `SetChildren`; this is just a convenience for tests / dev-tool
    /// tree-dump roots before the first topology batch lands.
    pub fn root(&self) -> Option<ElementNodeId> {
        self.root
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Read the last-applied paint state for `id` (dev-tool queries).
    pub fn node(&self, id: ElementNodeId) -> Option<MainNodeRef<'_>> {
        self.nodes.get(&id).map(|n| MainNodeRef {
            transform: n.transform,
            size: n.size,
            ops: &n.ops,
            child_ids: &n.child_ids,
        })
    }

    /// Last cursor claim received, if any.
    pub fn last_cursor(&self) -> Option<Cursor> {
        self.last_cursor
    }

    /// Drop everything. Called when the instance is destroyed so a
    /// reused `MainTree` doesn't bleed state across instances.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
        self.last_cursor = None;
    }
}

/// Read-only view of a `MainNode` (no clone needed for queries).
#[derive(Debug, Clone, Copy)]
pub struct MainNodeRef<'a> {
    pub transform: Affine,
    pub size: Size,
    pub ops: &'a [CanvasOp],
    pub child_ids: &'a [ElementNodeId],
}

/// Build the worker-side [`RenderCommand`] batch's topology portion
/// (`SetChildren` + `Remove`) by diffing the current element-tree topology
/// against the previous frame's.
///
/// `SetChildren` is emitted **only when a node's child list changed**
/// (diff against `last_topology` — steady-state frames emit zero topology
/// commands). `Remove` is emitted for any id present in `last_topology`
/// but missing from `current_ids`.
///
/// `child_ids_of` is called for each id in `current_ids` to fetch its
/// flattened children. Callers typically pass a closure that borrows the
/// element tree.
pub fn build_topology_batch(
    current_ids: &[ElementNodeId],
    child_ids_of: impl Fn(ElementNodeId) -> Vec<ElementNodeId>,
    last_topology: &mut HashMap<ElementNodeId, Vec<ElementNodeId>>,
) -> Vec<RenderCommand> {
    let current_set: HashSet<ElementNodeId> = current_ids.iter().copied().collect();

    let mut batch: Vec<RenderCommand> = Vec::new();

    // Removes: ids in last_topology but not in current.
    for id in last_topology.keys() {
        if !current_set.contains(id) {
            batch.push(RenderCommand::Remove { id: *id });
        }
    }
    last_topology.retain(|id, _| current_set.contains(id));

    // SetChildren: diff — emit only when a node's child list changed
    // (steady-state animation frames ship zero topology commands).
    for &id in current_ids {
        let child_ids = child_ids_of(id);
        let changed = last_topology.get(&id) != Some(&child_ids);
        if changed {
            last_topology.insert(id, child_ids.clone());
            batch.push(RenderCommand::SetChildren { id, child_ids });
        }
    }

    batch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::{Geometry, Size};
    use crate::core::render::brush::{Brush, Color};

    fn nid(n: u64) -> ElementNodeId {
        ElementNodeId::new(n)
    }

    fn fill_op() -> CanvasOp {
        CanvasOp::FillGeometry {
            offset: crate::core::layout::Offset::ZERO,
            geometry: Geometry::Rect(Size::new(10.0, 10.0)),
            brush: Brush::SolidColor(Color::rgb(255, 0, 0)),
        }
    }

    #[test]
    fn apply_batch_paint_creates_node_and_sets_root() {
        let mut tree = MainTree::new();
        tree.apply_batch(&[RenderCommand::Paint {
            id: nid(1),
            transform: Affine::IDENTITY,
            size: Size::new(10.0, 10.0),
            ops: vec![fill_op()],
        }]);

        assert_eq!(tree.root(), Some(nid(1)));
        assert_eq!(tree.node_count(), 1);
        let node = tree.node(nid(1)).expect("node 1 present");
        assert_eq!(node.ops.len(), 1);
    }

    #[test]
    fn apply_batch_setchildren_records_topology() {
        let mut tree = MainTree::new();
        tree.apply_batch(&[
            RenderCommand::SetChildren {
                id: nid(1),
                child_ids: vec![nid(2), nid(3)],
            },
            RenderCommand::SetChildren {
                id: nid(2),
                child_ids: vec![],
            },
        ]);

        assert_eq!(tree.node(nid(1)).unwrap().child_ids, vec![nid(2), nid(3)]);
        assert!(tree.node(nid(2)).unwrap().child_ids.is_empty());
    }

    #[test]
    fn apply_batch_remove_drops_node() {
        let mut tree = MainTree::new();
        tree.apply_batch(&[
            RenderCommand::SetChildren {
                id: nid(1),
                child_ids: vec![nid(2)],
            },
            RenderCommand::SetChildren {
                id: nid(2),
                child_ids: vec![],
            },
        ]);
        assert_eq!(tree.node_count(), 2);

        tree.apply_batch(&[RenderCommand::Remove { id: nid(2) }]);
        assert_eq!(tree.node_count(), 1);
        assert!(tree.node(nid(2)).is_none());
    }

    #[test]
    fn apply_batch_remove_root_clears_root_marker() {
        let mut tree = MainTree::new();
        tree.apply_batch(&[RenderCommand::Paint {
            id: nid(1),
            transform: Affine::IDENTITY,
            size: Size::new(10.0, 10.0),
            ops: vec![fill_op()],
        }]);
        assert_eq!(tree.root(), Some(nid(1)));

        tree.apply_batch(&[RenderCommand::Remove { id: nid(1) }]);
        assert_eq!(tree.root(), None);
    }

    #[test]
    fn apply_batch_cursor_updates_last_cursor() {
        let mut tree = MainTree::new();
        tree.apply_batch(&[RenderCommand::Cursor {
            cursor: Cursor::Pointer,
        }]);
        assert_eq!(tree.last_cursor(), Some(Cursor::Pointer));

        tree.apply_batch(&[RenderCommand::Cursor {
            cursor: Cursor::Text,
        }]);
        assert_eq!(tree.last_cursor(), Some(Cursor::Text));
    }

    #[test]
    fn build_topology_batch_diff_and_remove() {
        let mut last: HashMap<ElementNodeId, Vec<ElementNodeId>> = HashMap::new();
        // Pretend the previous frame had nodes 1, 2, 99.
        last.insert(nid(1), vec![]);
        last.insert(nid(2), vec![]);
        last.insert(nid(99), vec![]);

        let current = vec![nid(1), nid(2), nid(3)];
        let batch = build_topology_batch(
            &current,
            |id| match id {
                x if x == nid(1) => vec![nid(2), nid(3)],
                _ => vec![],
            },
            &mut last,
        );

        // 1 Remove (id=99) + 2 SetChildren (id=1 changed, id=3 new;
        // id=2 unchanged → no diff entry).
        let removes = batch
            .iter()
            .filter(|c| matches!(c, RenderCommand::Remove { .. }))
            .count();
        let sets = batch
            .iter()
            .filter(|c| matches!(c, RenderCommand::SetChildren { .. }))
            .count();
        assert_eq!(removes, 1, "got {batch:?}");
        assert_eq!(sets, 2, "got {batch:?}");

        // last_topology should now be exactly the current set.
        assert!(!last.contains_key(&nid(99)));
        assert!(last.contains_key(&nid(3)));

        // Steady state: a second batch with no changes emits nothing.
        let batch2 = build_topology_batch(
            &current,
            |id| match id {
                x if x == nid(1) => vec![nid(2), nid(3)],
                _ => vec![],
            },
            &mut last,
        );
        assert!(
            batch2.is_empty(),
            "steady-state topology must be empty, got {batch2:?}"
        );
    }

    #[test]
    fn clear_resets_all_state() {
        let mut tree = MainTree::new();
        tree.apply_batch(&[
            RenderCommand::Paint {
                id: nid(1),
                transform: Affine::IDENTITY,
                size: Size::new(10.0, 10.0),
                ops: vec![fill_op()],
            },
            RenderCommand::Cursor {
                cursor: Cursor::Pointer,
            },
        ]);
        assert_eq!(tree.node_count(), 1);
        assert!(tree.last_cursor().is_some());

        tree.clear();
        assert_eq!(tree.node_count(), 0);
        assert!(tree.root().is_none());
        assert!(tree.last_cursor().is_none());
    }
}
