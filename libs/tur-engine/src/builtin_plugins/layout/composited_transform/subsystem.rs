//! `CompositedTransformSubsystem` — recomputes each follower's position every
//! flush so it tracks its target through layout / scroll / reactive / transform
//! changes.
//!
//! For each active link it composes the target's full world affine (ancestor
//! offsets + paint transforms), maps the target anchor into canvas space, and
//! sets the follower's `computed_layout.offset` so that the follower is
//! translated to align `followerAnchor` with `targetAnchor` (+ `targetOffset`,
//! in the target's local space). Because the follower is repositioned via its
//! layout offset (a pure translation), both paint and hit-testing work through
//! the engine's normal offset accumulation — the follower and its descendants
//! are interactive exactly where they are painted.

use std::cell::RefCell;
use std::rc::Rc;

use vello_common::kurbo::Affine;

use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::layout::Offset;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

use super::follower::FollowerElement;
use super::link::CompositedLinkState;

pub struct CompositedTransformSubsystem {
    /// Shared with the `createLayerLink` closure. O(active links) per flush.
    pub(super) links: Rc<RefCell<Vec<Rc<CompositedLinkState>>>>,
}

/// A pending offset write computed during the read pass, applied in the write
/// pass (so the tree is borrowed immutably while reading, then mutably once).
struct PendingWrite {
    follower_id: ElementNodeId,
    new_offset: Offset,
    changed: bool,
}

impl Subsystem for CompositedTransformSubsystem {
    fn flush(&mut self, cx: &mut SubsystemFlushContext<'_>) {
        if self.links.borrow().is_empty() {
            return;
        }
        let snapshot: Vec<Rc<CompositedLinkState>> = self.links.borrow().clone();

        let mut pending: Vec<PendingWrite> = Vec::new();
        let mut any_changed = false;
        let mut keep: Vec<Rc<CompositedLinkState>> = Vec::new();

        {
            let tree = cx.element_tree.borrow();

            for state in &snapshot {
                let has_target = state.target_node.get().is_some();
                let has_follower = state.follower_node.get().is_some();
                if !has_target && !has_follower {
                    // Both ends gone — drop from the registry.
                    continue;
                }
                keep.push(state.clone());

                let Some(target_id) = state.target_node.get() else {
                    state.linked.set(false);
                    continue;
                };
                let Some(target_node) = tree.get_element(target_id) else {
                    // Target vanished (e.g. Condition flipped it off).
                    state.linked.set(false);
                    continue;
                };

                let target_size = target_node.computed_layout.size;
                let target_world = compose_world_affine(&tree, target_id);
                state.target_world.set(target_world);
                state.target_size.set(target_size);
                state.linked.set(true);

                let Some(follower_id) = state.follower_node.get() else {
                    continue;
                };
                let Some(follower_node) = tree.get_element(follower_id) else {
                    continue;
                };
                let Some(element) = follower_node.element.as_ref() else {
                    continue;
                };
                let Some(follower) = element.cast::<FollowerElement>() else {
                    continue;
                };

                let desired = follower.desired_origin(
                    target_world,
                    target_size,
                    follower_node.computed_layout.size,
                );

                // Reconstruct ancestor-only accumulated offset (excluding the
                // follower's own) from its current absolute position, so the
                // relative offset we write lands the follower at `desired`.
                let current_abs = tree.absolute_offset_of(follower_id);
                let current_rel = follower_node.computed_layout.offset;
                let ancestor_abs = Offset::new(
                    current_abs.x - current_rel.x,
                    current_abs.y - current_rel.y,
                );
                let new_offset = Offset::new(desired.x - ancestor_abs.x, desired.y - ancestor_abs.y);
                let changed = (new_offset.x - current_rel.x).abs() > 1e-9
                    || (new_offset.y - current_rel.y).abs() > 1e-9;
                if changed {
                    any_changed = true;
                }
                pending.push(PendingWrite {
                    follower_id,
                    new_offset,
                    changed,
                });
            }
        }

        // Compact the registry (drop fully-detached links).
        if keep.len() != snapshot.len() {
            *self.links.borrow_mut() = keep;
        }

        // Apply the offset writes in a single mutable borrow.
        if !pending.is_empty() {
            let mut tree = cx.element_tree.borrow_mut();
            for w in &pending {
                if let Some(node) = tree.get_element_mut(w.follower_id) {
                    node.computed_layout.offset = w.new_offset;
                }
            }
        }

        if any_changed {
            cx.request_paint();
        }
        // Keep `changed` per-write meaningful for future diagnostics.
        let _ = pending.iter().any(|w| w.changed);
    }
}

/// Compose the full world affine mapping `node`-local points to canvas space:
/// the product of `translate(offset) * paint_transform` for the node and every
/// ancestor, root→leaf. Includes ancestor `Transform` affines (via
/// [`crate::core::render::ElementRender::paint_transform`]).
fn compose_world_affine(
    tree: &crate::core::elements::NodeTreeData,
    id: ElementNodeId,
) -> Affine {
    // Collect the chain node → root (hopping through zero-offset fragments).
    let mut chain: Vec<ElementNodeId> = Vec::new();
    let mut cursor: Option<NodeId> = Some(id.into());
    while let Some(cid) = cursor {
        let eid = ElementNodeId::new(cid.as_u64());
        let fid = FragmentNodeId::new(cid.as_u64());
        if let Some(n) = tree.get_element(eid) {
            chain.push(eid);
            cursor = n.parent;
        } else if let Some(f) = tree.get_fragment(fid) {
            cursor = Some(f.parent);
        } else {
            break;
        }
    }
    // Fold root → leaf.
    let mut world = Affine::IDENTITY;
    for eid in chain.into_iter().rev() {
        let Some(n) = tree.get_element(eid) else {
            continue;
        };
        let layout = n.computed_layout;
        let local_translate = Affine::translate((layout.offset.x, layout.offset.y));
        let local_paint = n
            .element
            .as_ref()
            .and_then(|e| e.paint_transform(&layout))
            .unwrap_or(Affine::IDENTITY);
        world = world * local_translate * local_paint;
    }
    world
}
