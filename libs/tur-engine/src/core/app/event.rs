//! Engine-internal event bus.
//!
//! Carries requests produced by elements and handlers during a flush
//! (programmatic scrolls, domain-specific requests such as clipboard writes
//! / paste forwarding) and consumed by other handlers via
//! `Subsystem::handle_app_event` within the same fixed-point flush loop.
//! These never cross the embedder boundary.
//!
//! Paint requests do **not** live here — they set the `need_paint` flag
//! directly (see
//! [`TurInstanceContext::need_paint`](crate::core::js_runtime::TurInstanceContext)).
//!
//! This is also where **derived** scrolling lives: when the gesture arena
//! resolves a touch drag to scroll it emits [`AppEvent::Scroll`] here (rather
//! than faking a `ShellEventPayload::Wheel`), so the wheel handler can process
//! real and derived scroll through one path.
//!
//! Domain-specific app events (e.g. clipboard write / paste) travel inside
//! [`AppEvent::Custom`] as [`CustomAppEvent`] payloads, keeping the engine
//! free of per-domain variant knowledge.

use crate::core::element::{ElementNodeId, ViewRootId};
use crate::core::layout::Offset;

/// Trait implemented by payload types carried inside an [`AppEvent::Custom`].
/// Capability crates use this to inject their own engine-internal event types
/// (e.g. clipboard write requests, forwarded paste requests) without forcing
/// the engine to know about them.
///
/// Implementors also expose [`Any`](std::any::Any) for downcasting so
/// consumers can recover the concrete payload type via
/// [`AppEvent::as_custom`].
///
/// `Send + Sync` is required so an `AppEvent` can cross the worker↔main
/// channel boundary (Phase 4+). All current implementors are plain data
/// (`{ text: String }`, `{ vx, vy, position }`); the bound is a
/// forward-looking guard.
pub trait CustomAppEvent: std::any::Any + std::fmt::Debug + Send + Sync {
    /// Stable, human-readable identifier used for diagnostics / tracing.
    fn name(&self) -> &'static str;
    /// Borrow as `&dyn Any` so the dispatcher can downcast without leaking
    /// the concrete type to the engine.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Engine-internal event bus. See the [module docs](self) for full semantics.
pub enum AppEvent {
    /// Programmatic scroll request — set the absolute scroll offset of the
    /// target scroll-view node. Emitted by scrollbar drag (where the gesture
    /// handler can't mutate the tree directly due to an active borrow).
    ScrollTo { node_id: ElementNodeId, offset: f64 },
    /// Scroll overflow bubbling — a scroll view consumed as much delta as it
    /// could and is forwarding the remainder (`delta`) to its nearest
    /// scrollable ancestor. Resolved by `ScrollSubsystem`.
    ScrollOverscroll {
        source_id: ElementNodeId,
        delta: f64,
    },
    /// Derived scroll delta produced by the gesture arena (e.g. a touch drag
    /// that the arena resolved to scroll), targeting the view root the
    /// touch landed on. Consumed by the wheel handler and routed through
    /// the exact same pipeline as a real
    /// [`ShellEventPayload::Wheel`](crate::core::platform::ShellEventPayload::Wheel).
    Scroll {
        root: ViewRootId,
        delta_x: f64,
        delta_y: f64,
        position: Offset,
    },
    /// Domain-specific engine-internal event (e.g. clipboard write / paste
    /// forwarding). Capability crates define their own payload types
    /// implementing [`CustomAppEvent`]; consumers downcast via
    /// [`AppEvent::as_custom`].
    Custom(Box<dyn CustomAppEvent>),
}

impl AppEvent {
    /// Wrap a [`CustomAppEvent`] payload in the [`AppEvent::Custom`] variant.
    pub fn custom<T: CustomAppEvent>(payload: T) -> Self {
        Self::Custom(Box::new(payload))
    }

    /// If this event is an [`AppEvent::Custom`] carrying a payload of type
    /// `T`, borrow the payload; otherwise `None`.
    pub fn as_custom<T: CustomAppEvent>(&self) -> Option<&T> {
        if let Self::Custom(p) = self {
            p.as_any().downcast_ref::<T>()
        } else {
            None
        }
    }
}
