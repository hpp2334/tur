//! `CompositedTransformSubsystem` — recomputes each follower's tracked
//! transform every flush so it tracks its target through layout / scroll /
//! reactive / transform changes.
//!
//! For each active link it composes the target's full world affine (ancestor
//! `relative_transform`s), maps the target anchor into canvas space, and writes
//! the follower's tracked **relative affine** onto the **link**
//! (`follower_transform`). The follower returns that affine verbatim from its
//! `relative_transform`, so paint, hit-test, and bounds all resolve to the
//! tracked position. The tracked transform lives on the link (single owner:
//! this subsystem) — never in `computed_layout.offset` (which layout owns) — so
//! a parent relayout can't clobber it and there is no layout/subsystem fight
//! (no "flash to top-left").
//!
//! This recomputation runs in [`Subsystem::flush_post_layout`] (after the
//! layout step of each fixed-point iteration) so it reads **fresh** target +
//! follower geometry (`computed_layout.size`, `absolute_affine_of`) and the
//! follower's just-resolved anchor cache. Running it pre-layout left it reading
//! zero/stale sizes on the first frame, so a follower with non-`TopLeft` anchors
//! painted at the wrong offset until the next input event triggered a fresh
//! flush.

use std::cell::RefCell;
use std::rc::Rc;

use vello_common::kurbo::Affine;

use crate::core::element::ElementNodeId;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

use super::follower::FollowerElement;
use super::link::CompositedLinkState;

pub struct CompositedTransformSubsystem {
    /// Shared with the `createLayerLink` closure. O(active links) per flush.
    pub(super) links: Rc<RefCell<Vec<Rc<CompositedLinkState>>>>,
}

impl Subsystem for CompositedTransformSubsystem {
    fn flush_post_layout(&mut self, cx: &mut SubsystemFlushContext<'_>) {
        if self.links.borrow().is_empty() {
            return;
        }
        let snapshot: Vec<Rc<CompositedLinkState>> = self.links.borrow().clone();

        let mut any_changed = false;
        let mut keep: Vec<Rc<CompositedLinkState>> = Vec::new();

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
            let target_world = tree.absolute_affine_of(target_id);
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

            // Solve the follower's relative affine: we want the follower's
            // world origin at `desired`, so compose `parent_world⁻¹ ·
            // translate(desired)`. Using the full parent affine (not summed
            // offsets) means a follower living under a rotated/scaled
            // `Transform` is tracked correctly — the inverse maps the desired
            // world point back into the parent's frame, exactly as
            // `relative_transform` will compose it during paint/hit-test.
            let parent_world = follower_node
                .parent
                .map(|pid| tree.absolute_affine_of(ElementNodeId::new(pid.as_u64())))
                .unwrap_or(Affine::IDENTITY);
            let new_transform =
                parent_world.inverse() * Affine::translate((desired.x, desired.y));

            let prev = state.follower_transform.get();
            let pm = prev.as_coeffs();
            let nm = new_transform.as_coeffs();
            let changed = pm.iter().zip(nm.iter()).any(|(a, b)| (a - b).abs() > 1e-9);
            // Write the tracked transform onto the link (single owner). No
            // tree mutation — the follower reads this via `relative_transform`.
            state.follower_transform.set(new_transform);
            if changed {
                any_changed = true;
            }
        }

        // Compact the registry (drop fully-detached links).
        if keep.len() != snapshot.len() {
            *self.links.borrow_mut() = keep;
        }

        if any_changed {
            cx.request_paint();
        }
    }
}
