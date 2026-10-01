//! Pointer-input plugin: gesture arena + composer + pointer-region tracker,
//! plus the `MouseRegion` + `PointerInteract` elements that consume them.
//!
//! - `GestureSubsystem` — owns the gesture arena, composes click/long-press
//!   /drag from raw pointer events, dispatches composed gesture events to
//!   `PointerInteract` elements.
//! - `PointerSubsystem` — tracks `onEnter`/`onExit` for `MouseRegion` as the
//!   pointer moves; cursor resolution still happens in the paint pass.

pub(in crate::builtin_plugins) mod gesture_handler;
pub mod mouse_region;
pub(in crate::builtin_plugins) mod pointer_interact;
pub(in crate::builtin_plugins) mod pointer_region_handler;
pub(in crate::builtin_plugins) mod pointer_region_tracker;

// Temporary: tur-integration-tests (still external until Phase H rewrites
// those tests to use JS+dev-tool queries) constructs these element types
// directly. After Phase H, this re-export goes away.
pub use gesture_handler::GestureSubsystem;
pub(crate) mod rut_rows;

pub use mouse_region::{MouseRegionElement, MouseRegionView, PointerRegionEvent};
pub use pointer_interact::{PointerInteractElement, PointerInteractEvent, PointerInteractView};
pub use pointer_region_handler::PointerSubsystem;

use crate::core::plugin::PluginRegisterContext;
use crate::error::TurError;

/// Install the gesture plugin (`MouseRegion`, `PointerInteract`) and
/// register `GestureSubsystem` + `PointerSubsystem`.
pub fn install_gesture(ctx: &mut PluginRegisterContext) -> Result<(), TurError> {
    // The gesture families' `tur` rows (the kit wraps them):
    // PointerInteract / MouseRegion / Focusable.
    ctx.push_rut_ext(std::rc::Rc::new(rut_rows::install_ext));
    ctx.register_subsystem(Box::new(gesture_handler::GestureSubsystem::new()));
    ctx.register_subsystem(Box::new(pointer_region_handler::PointerSubsystem::new()));
    Ok(())
}
