//! `CompositedTransformSubsystem` — recomputes each follower's tracked
//! position every flush so it tracks its target through layout / scroll /
//! reactive / transform changes.
//!
//! For each active link it composes the target's full world affine (ancestor
//! `relative_transform`s), maps the target anchor into canvas space, and writes
//! the follower's tracked offset onto the **link** (`follower_offset`,
//! parent-relative). The follower exposes that offset via its
//! `relative_transform` (a pure translation), so paint, hit-test, and bounds
//! all resolve to the tracked position. The tracked offset lives on the link
//! (single owner: this subsystem) — never in `computed_layout.offset` (which
//! layout owns) — so a parent relayout can't clobber it and there is no
//! layout/subsystem offset fight (no "flash to top-left").

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::layout::Offset;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

use super::follower::FollowerElement;
use super::link::CompositedLinkState;

pub struct CompositedTransformSubsystem {
    /// Shared with the `createLayerLink` closure. O(active links) per flush.
    pub(super) links: Rc<RefCell<Vec<Rc<CompositedLinkState>>>>,
}

impl Subsystem for CompositedTransformSubsystem {
    fn flush(&mut self, cx: &mut SubsystemFlushContext<'_>) {
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

            // Convert the canvas-space `desired` origin to a PARENT-relative
            // offset (the form `relative_transform` consumes) by subtracting
            // the follower's ancestor accumulated offset. `computed_layout.offset`
            // is layout-owned (typically (0,0) for an overlay slot) and is NOT
            // read for tracking — only the ancestor chain matters here.
            let current_abs = tree.absolute_offset_of(follower_id);
            let current_rel = follower_node.computed_layout.offset;
            let ancestor_abs = Offset::new(
                current_abs.x - current_rel.x,
                current_abs.y - current_rel.y,
            );
            let new_offset = Offset::new(desired.x - ancestor_abs.x, desired.y - ancestor_abs.y);

            let prev = state.follower_offset.get();
            let changed = (new_offset.x - prev.x).abs() > 1e-9
                || (new_offset.y - prev.y).abs() > 1e-9;
            // Write the tracked offset onto the link (single owner). No tree
            // mutation — the follower reads this via `relative_transform`.
            state.follower_offset.set(new_offset);
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
