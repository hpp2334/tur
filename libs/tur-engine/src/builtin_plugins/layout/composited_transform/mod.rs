//! `CompositedTransformTarget` / `CompositedTransformFollower` — Flutter-style
//! anchor linking. A follower renders at a target's global position (tracked
//! continuously through layout / scroll / reactive / transform changes), used
//! for tooltips, dropdowns, popovers.
//!
//! ## Model
//!
//! - A [`LayerLink`] is a shared handle created via `createLayerLink()` and
//!   passed to one target + one follower.
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

mod follower;
mod link;
mod subsystem;
mod target;

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginRegisterContext;
use crate::error::TurError;

use link::CompositedLinkState;
use subsystem::CompositedTransformSubsystem;

/// Per-instance plugin state: the shared registry of active links — held by
/// the subsystem (per-flush recompute) and readable by the
/// `createLayerLink` bridge fn through the instance ctx (`args[0]`), so the
/// fn is a plain ctx-bound `FnEntry` pointer (no closures). O(active links)
/// per flush.
pub(crate) struct LayerLinkRegistry(pub Rc<RefCell<Vec<Rc<CompositedLinkState>>>>);

/// Install the composited-transform elements + the link factory + the
/// tracking subsystem.
///
/// Returns the `FnEntry`s (`CompositedTransformTarget`,
/// `CompositedTransformFollower`, `createLayerLink`) — all plain ctx-bound
/// fn pointers; the shared link registry rides the register-phase
/// plugin-state channel.
pub fn install_composited_transform(
    ctx: &mut PluginRegisterContext<'_>,
) -> Result<Vec<FnEntry>, TurError> {
    let links: Rc<RefCell<Vec<Rc<CompositedLinkState>>>> = Rc::new(RefCell::new(Vec::new()));

    ctx.register_subsystem(Box::new(CompositedTransformSubsystem {
        links: links.clone(),
    }));

    ctx.define_plugin_state(Rc::new(LayerLinkRegistry(links)));

    Ok(vec![
        (
            "CompositedTransformTarget",
            2,
            target::tur_target as crate::core::js_runtime::helpers::Ptr,
        ),
        (
            "CompositedTransformFollower",
            2,
            follower::tur_follower as crate::core::js_runtime::helpers::Ptr,
        ),
        (
            "createLayerLink",
            2,
            link::tur_create_layer_link as crate::core::js_runtime::helpers::Ptr,
        ),
    ])
}
