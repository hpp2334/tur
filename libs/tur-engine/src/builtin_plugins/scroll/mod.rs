//! Scroll plugin — scrollable container elements (`ScrollViewElement`,
//! `ScrollbarElement`), the plain-Rust [`ScrollController`], the
//! [`ScrollSubsystem`] event-pipeline participant, and the shared scroll
//! primitives (`ScrollEvent`, `ScrollPosition`, `ScrollPhysics`).
//!
//! Installed by `TurStdPlugin` via [`install_scroll`], which registers the
//! scroll subsystems. Controllers are pure Rust (`Rc<RefCell<...>>`) —
//! bound to their element at view-build time by `ScrollViewView::build`;
//! `RustUi` authors them through the `core::rut_runtime` rows.
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

use crate::core::plugin::PluginRegisterContext;
use crate::error::TurError;

pub use self::core::ScrollEvent;
pub use self::core::controller::ScrollController;
pub use self::handlers::{ScrollInertiaSubsystem, ScrollSubsystem, dispatch_wheel};
pub use self::scroll_view::{ScrollPhysics, ScrollPosition, ScrollViewElement, ScrollViewView};
pub use self::scrollbar::{ScrollbarElement, ScrollbarView};

/// Wire the scroll plugin's subsystems in. Called by `TurStdPlugin`'s
/// `register` impl.
///
/// Side effects:
/// - Registers [`ScrollSubsystem`] (consumes `ShellEvent::Wheel`,
///   `AppEvent::Scroll` / `ScrollTo` / `ScrollOverscroll`; owns wheel
///   dispatch, overscroll chaining, and programmatic scroll-to).
/// - Registers [`ScrollInertiaSubsystem`] after `ScrollSubsystem`, so
///   fling-seed events (which arrive via `handle_app_event`) are processed
///   after the gesture plugin pushes them on touch-up. Captures the engine
///   clock so it can integrate exponential decay each `flush`.
pub fn install_scroll(ctx: &mut PluginRegisterContext) -> Result<(), TurError> {
    ctx.register_subsystem(Box::new(ScrollSubsystem));
    // Registered after `ScrollSubsystem` so fling-seed events (which arrive
    // via `handle_app_event`) are processed after the gesture plugin pushes
    // them on touch-up. Captures the engine clock so it can integrate
    // exponential decay each `flush`.
    ctx.register_subsystem(Box::new(ScrollInertiaSubsystem::new(ctx.clock())));
    Ok(())
}
