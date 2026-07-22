//! Scroll subsystem for tur.
//!
//! Provides the scrollable container elements (`ScrollViewElement`,
//! `ScrollbarElement`), their controllers (`ScrollController`), the
//! `ScrollSubsystem` event-pipeline participant, and the shared scroll
//! primitives (`ScrollEvent`, `ScrollPosition`, `ScrollPhysics`).
//!
//! Like `tur-text`, this crate is **not** a standalone plugin. It is installed
//! into `tur:std` by `TurStdPlugin` via [`install_scroll_feature`],
//! which registers the boa class + subsystem and returns the JS factory fns to
//! be merged into `std_fns`. From JS's perspective `ScrollView`,
//! `createScrollController`, and `Scrollbar` ship as part of `tur:std`.
//!
//! The engine retains the event protocol — `AppEvent::Scroll`,
//! `AppEvent::ScrollTo`, `AppEvent::ScrollOverscroll` and the
//! `request_scroll_to` gesture producer — which [`handlers::ScrollSubsystem`]
//! consumes here. The `WheelEvent` type and `AnyElement::with_wheel(...)` /
//! `with_gesture_and_focus(...)` builders live in the engine too.

pub mod core;
pub mod handlers;
pub mod scroll_view;
pub mod scrollbar;

pub use core::controller::ScrollController;
pub use core::ScrollEvent;
pub use handlers::{dispatch_wheel, ScrollSubsystem};
pub use scroll_view::{ScrollPhysics, ScrollPosition, ScrollViewElement, ScrollViewView};
pub use scrollbar::{ScrollbarElement, ScrollbarView};

use tur_engine::core::bridge::helpers::FnEntry;
use tur_engine::core::plugin::PluginContext;
use tur_engine::error::TurError;

/// Wire the scroll feature into `tur:std`. Called by `TurStdPlugin`'s
/// `register` impl — scroll is not a separate plugin, it's a feature installed
/// into the std module.
///
/// Side effects:
/// - Registers the boa class [`ScrollController`] on `globalThis`.
/// - Registers [`ScrollSubsystem`] (consumes `PlatformEvent::Wheel`,
///   `AppEvent::Scroll` / `ScrollTo` / `ScrollOverscroll`; owns wheel
///   dispatch, overscroll chaining, and programmatic scroll-to).
///
/// Returns: the `ScrollView` / `createScrollController` / `Scrollbar` factory
/// fns, which the caller merges into `std_fns` before
/// `register_module("tur:std", ...)`.
pub fn install_scroll_feature(
    ctx: &mut PluginContext<'_>,
) -> Result<Vec<FnEntry>, TurError> {
    ctx.register_class::<ScrollController>()
        .map_err(|e| TurError::Other(format!("failed to register ScrollController: {e}")))?;
    ctx.register_subsystem(Box::new(ScrollSubsystem));

    let mut fns = Vec::new();
    fns.extend(scroll_view::bridge::fns());
    fns.extend(scrollbar::bridge::fns());
    Ok(fns)
}
