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

use boa_engine::native_function::NativeFunction;

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginContext;
use crate::error::TurError;

use link::CompositedLinkState;
use subsystem::CompositedTransformSubsystem;

/// A JS-side closure entry: `(js_name, length, native_function)`.
pub(crate) type ClosureEntry = (&'static str, usize, NativeFunction);

/// Install the composited-transform elements + the link factory + the
/// tracking subsystem.
///
/// Returns the element factory `FnEntry`s (`CompositedTransformTarget`,
/// `CompositedTransformFollower`) and the `createLayerLink` closure (which
/// captures the shared link registry also held by the subsystem).
pub fn install_composited_transform(
    ctx: &mut PluginContext<'_>,
) -> Result<(Vec<FnEntry>, Vec<ClosureEntry>), TurError> {
    // Shared registry of active links — owned by the subsystem and captured
    // by the `createLayerLink` closure. O(active links) per flush.
    let links: Rc<RefCell<Vec<Rc<CompositedLinkState>>>> = Rc::new(RefCell::new(Vec::new()));

    ctx.register_subsystem(Box::new(CompositedTransformSubsystem {
        links: links.clone(),
    }));

    let create_layer_link = link::build_create_layer_link(links);

    let fns = vec![
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
    ];
    let closures = vec![("createLayerLink", 0, create_layer_link)];

    Ok((fns, closures))
}
