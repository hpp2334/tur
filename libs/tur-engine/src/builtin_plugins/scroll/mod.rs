//! Scroll plugin — scrollable container elements (`ScrollViewElement`,
//! `ScrollbarElement`), their boa class (`ScrollController`), the
//! `ScrollSubsystem` event-pipeline participant, and the shared scroll
//! primitives (`ScrollEvent`, `ScrollPosition`, `ScrollPhysics`).
//!
//! Installed into `tur:std` by `TurStdPlugin` via
//! [`install_scroll`], which registers the boa class + subsystem and returns
//! the JS factory fns to be merged into `std_fns`. From JS's perspective
//! `ScrollView`, `createScrollController`, and `Scrollbar` ship as part of
//! `tur:std`.
//!
//! The engine retains the event protocol — `AppEvent::Scroll`,
//! `AppEvent::ScrollTo`, `AppEvent::ScrollOverscroll` and the
//! `request_scroll_to` gesture producer — which [`handlers::ScrollSubsystem`]
//! consumes here. The `WheelEvent` type and `AnyElement::with_wheel(...)` /
//! `with_gesture_and_focus(...)` builders live in the engine too.

pub mod core;
pub mod event;
pub mod handlers;
pub mod scroll_view;
pub mod scrollbar;

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginContext;
use crate::error::TurError;

pub use self::core::ScrollEvent;
pub use self::core::controller::ScrollController;
pub use self::handlers::{ScrollInertiaSubsystem, ScrollSubsystem, dispatch_wheel};
pub use self::scroll_view::{ScrollPhysics, ScrollPosition, ScrollViewElement, ScrollViewView};
pub use self::scrollbar::{ScrollbarElement, ScrollbarView};

/// Wire the scroll plugin into `tur:std`. Called by `TurStdPlugin`'s
/// `register` impl.
///
/// Side effects:
/// - Registers the boa class [`ScrollController`] on `globalThis`.
/// - Registers [`ScrollSubsystem`] (consumes `ShellEventPayload::Wheel`,
///   `AppEvent::Scroll` / `ScrollTo` / `ScrollOverscroll`; owns wheel
///   dispatch, overscroll chaining, and programmatic scroll-to).
///
/// Returns: the `ScrollView` / `createScrollController` / `Scrollbar` factory
/// fns, which the caller merges into `std_fns` before
/// `register_module("tur:std", ...)`.
pub fn install_scroll(ctx: &mut PluginContext<'_>) -> Result<Vec<FnEntry>, TurError> {
    ctx.register_class::<ScrollController>()
        .map_err(|e| TurError::Other(format!("failed to register ScrollController: {e}")))?;
    ctx.register_subsystem(Box::new(ScrollSubsystem));
    // Registered after `ScrollSubsystem` so fling-seed events (which arrive
    // via `handle_app_event`) are processed after the gesture plugin pushes
    // them on touch-up. Captures the engine clock so it can integrate
    // exponential decay each `flush`.
    ctx.register_subsystem(Box::new(ScrollInertiaSubsystem::new(ctx.clock())));

    let mut fns = Vec::new();
    fns.extend(scroll_view::bridge::fns());
    fns.extend(scrollbar::bridge::fns());
    Ok(fns)
}
