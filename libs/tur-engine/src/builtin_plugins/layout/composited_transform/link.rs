//! The shared `LayerLink` handle.
//!
//! Links are minted by the rut rows (`core::rut_runtime::composited`'s
//! `ct_link_new`) and registered into the [`LayerLinkRegistry`](super::LayerLinkRegistry)
//! plugin state — the same registry the tracking subsystem recomputes from.

use std::cell::Cell;
use std::rc::Rc;

use vello_common::kurbo::Affine;

use crate::core::element::ElementNodeId;
use crate::core::layout::Size;

/// Shared state backing a `LayerLink`. One per minted link; held by the
/// target element, the follower element, and the
/// [`super::subsystem::CompositedTransformSubsystem`] registry.
///
/// All fields are `Cell`/interior-mutable because the same `Rc` is shared
/// across the build phase (target/follower set their node ids), the subsystem
/// flush (writes the resolved geometry), and paint/hit-test (reads it).
#[derive(Debug)]
pub struct CompositedLinkState {
    /// Set by `CompositedTransformTarget::build`.
    pub target_node: Cell<Option<ElementNodeId>>,
    /// Set by `CompositedTransformFollower::build`.
    pub follower_node: Cell<Option<ElementNodeId>>,

    // --- resolved by the subsystem each flush, read by the follower ---
    /// The target's full world affine (ancestor offsets + paint transforms),
    /// mapping target-local points to canvas space.
    pub target_world: Cell<Affine>,
    /// The target's laid-out size (for resolving `targetAnchor`).
    pub target_size: Cell<Size>,
    /// The follower's tracked **relative transform** (the affine the follower
    /// should apply within its parent's frame), written by the subsystem each
    /// flush. The follower returns this verbatim from `relative_transform`, so
    /// paint, hit-test, and bounds all resolve to the tracked position WITHOUT
    /// storing it in `computed_layout.offset` (which layout owns). Single
    /// ownership — no two writers, so no "flash to top-left" oscillation.
    /// Stored as the full `Affine` (not an `Offset`) so the subsystem can solve
    /// `parent_world⁻¹ · translate(desired)` and correctly track through a
    /// rotated/scaled ancestor `Transform`.
    pub follower_transform: Cell<Affine>,
    /// `true` once at least one flush has resolved a valid target. The
    /// follower uses this to implement `showWhenUnlinked`.
    pub linked: Cell<bool>,
}

impl Default for CompositedLinkState {
    fn default() -> Self {
        Self {
            target_node: Cell::new(None),
            follower_node: Cell::new(None),
            target_world: Cell::new(Affine::IDENTITY),
            target_size: Cell::new(Size::ZERO),
            follower_transform: Cell::new(Affine::IDENTITY),
            linked: Cell::new(false),
        }
    }
}

/// A plain handle wrapping the shared state (no script-realm plumbing —
/// the rut rail's `RutLayerLink` opaque carries the same `Rc`).
#[derive(Debug, Clone)]
pub struct LayerLink(pub Rc<CompositedLinkState>);

impl LayerLink {
    pub fn new(state: Rc<CompositedLinkState>) -> Self {
        Self(state)
    }
}
