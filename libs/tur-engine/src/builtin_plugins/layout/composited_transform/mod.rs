//! `CompositedTransformTarget` / `CompositedTransformFollower` — Flutter-style
//! anchor linking. A follower renders at a target's global position (tracked
//! continuously through layout / scroll / reactive / transform changes), used
//! for tooltips, dropdowns, popovers.
//!
//! ## Model
//!
//! - A [`LayerLink`] is a shared handle minted by the rut rows
//!   (`ct_link_new`) and registered into the link registry.
//! - [`CompositedTransformTarget`] is a transparent passthrough that records
//!   its node id on the link.
//! - [`CompositedTransformFollower`] records its node id on the link and
//!   renders at the target's anchor; it should be placed in a root overlay
//!   slot (the Flutter `Overlay` pattern) so it isn't clipped and paints on
//!   top.
//! - [`CompositedTransformSubsystem`] runs each flush: it composes the
//!   target's full world affine (ancestor offsets + paint transforms), maps
//!   the target anchor into canvas space, and sets the follower's
//!   `computed_layout.offset` so the follower is *translated* to align
//!   `followerAnchor` with `targetAnchor` (+ `targetOffset`, expressed in the
//!   target's local space). Translation-only (the follower stays axis-aligned)
//!   matches Flutter's `CompositedTransformFollower`.
//!
//! Setting the follower's offset (rather than applying a paint transform)
//! means hit-testing works through the normal offset accumulation — the
//! follower and its descendants are hit-tested where they are painted.

pub mod follower;
pub mod link;
pub(crate) mod rut_rows;
mod subsystem;
pub mod target;

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::plugin::PluginRegisterContext;
use crate::error::TurError;

use link::CompositedLinkState;
use subsystem::CompositedTransformSubsystem;

/// Per-instance plugin state: the shared registry of active links — held by
/// the subsystem (per-flush recompute) and read by the rut rows'
/// `ct_link_new` (via the instance's plugin-state lookup), so every minted
/// link is tracked for the per-flush recompute. O(active links) per flush.
pub struct LayerLinkRegistry(pub Rc<RefCell<Vec<Rc<CompositedLinkState>>>>);

/// Install the composited-transform tracking subsystem + the shared link
/// registry (the plugin-state channel the rut rows' `ct_link_new` reads).
pub fn install_composited_transform(ctx: &mut PluginRegisterContext) -> Result<(), TurError> {
    let links: Rc<RefCell<Vec<Rc<CompositedLinkState>>>> = Rc::new(RefCell::new(Vec::new()));

    ctx.register_subsystem(Box::new(CompositedTransformSubsystem {
        links: links.clone(),
    }));

    ctx.define_plugin_state(Rc::new(LayerLinkRegistry(links)));

    // The composited families' `tur` rows (link / target / follower).
    ctx.push_rut_ext(std::rc::Rc::new(rut_rows::install_decl_ext));

    Ok(())
}
