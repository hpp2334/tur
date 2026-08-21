//! Platform-side event envelope: everything the embedder pushes into the
//! engine via [`TurApp::push_platform_event`](crate::TurApp::push_platform_event)
//! and dispatched to handlers via `Subsystem::handle_platform_event`.
//!
//! Two kinds of payload:
//! - [`PlatformEvent::Shell`] — raw window-system input (pointer, wheel,
//!   key, ime, resize) as [`ShellEvent`](crate::core::shell::ShellEvent).
//!   See the shell event module docs for the semantics (gestures are
//!   computed *inside* the engine, never faked as shell events).
//! - [`PlatformEvent::Custom`] — domain-specific platform events (e.g.
//!   clipboard paste from the embedder) as [`CustomPlatformEvent`]
//!   payloads, keeping the engine free of per-domain variant knowledge.

/// Trait implemented by payload types carried inside a
/// [`PlatformEvent::Custom`]. Capability crates use this to inject their own
/// platform-originated event types (e.g. clipboard paste from the embedder)
/// without forcing the engine to know about them.
///
/// Implementors also expose [`Any`](std::any::Any) for downcasting so
/// consumers can recover the concrete payload type via
/// [`PlatformEvent::as_custom`].
///
/// `Send + Sync` is required so a `PlatformEvent` can cross the worker↔main
/// channel boundary (Phase 4+). All current implementors are plain data
/// (`{ text: String }`, etc.); the bound is a forward-looking guard.
pub trait CustomPlatformEvent: std::any::Any + std::fmt::Debug + Send + Sync {
    /// Stable, human-readable identifier used for diagnostics / tracing.
    fn name(&self) -> &'static str;
    /// Borrow as `&dyn Any` so the dispatcher can downcast without leaking
    /// the concrete type to the engine.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Input events originating from the platform / embedder (window system,
/// browser DOM). See the [module docs](self) for the full semantics.
pub enum PlatformEvent {
    /// Raw window-system input — pointer / wheel / key / ime / resize —
    /// wrapped in the [`ShellEvent`](crate::core::shell::ShellEvent)
    /// envelope variant (the shell layer's ingress face).
    Shell(crate::core::shell::ShellEvent),
    /// Domain-specific platform event (e.g. clipboard paste from the
    /// embedder). Capability crates define their own payload types
    /// implementing [`CustomPlatformEvent`]; consumers downcast via
    /// [`PlatformEvent::as_custom`].
    Custom(Box<dyn CustomPlatformEvent>),
}

impl PlatformEvent {
    /// If this event is a [`PlatformEvent::Custom`] carrying a payload of
    /// type `T`, borrow the payload; otherwise `None`.
    pub fn as_custom<T: CustomPlatformEvent>(&self) -> Option<&T> {
        if let Self::Custom(p) = self {
            p.as_any().downcast_ref::<T>()
        } else {
            None
        }
    }
}

impl From<crate::core::shell::ShellEvent> for PlatformEvent {
    fn from(ev: crate::core::shell::ShellEvent) -> Self {
        Self::Shell(ev)
    }
}
