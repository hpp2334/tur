pub mod queue;

use crate::core::element::ElementNodeId;
use crate::core::keyboard::KeyEvent;
use crate::core::layout::{MouseButton, Offset};

/// The physical input device that produced a pointer event. Used by the
/// gesture arena to apply different disambiguation rules for touch vs
/// mouse — touch drags go through slop-based arena resolution (scroll
/// vs drag), while mouse events are dispatched immediately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerDeviceKind {
    Mouse,
    Touch,
}

/// Trait implemented by payload types carried inside a
/// [`PlatformEvent::Custom`]. Capability crates use this to inject their own
/// platform-originated event types (e.g. clipboard paste from the embedder)
/// without forcing the engine to know about them.
///
/// Implementors also expose [`Any`](std::any::Any) for downcasting so
/// consumers can recover the concrete payload type via
/// [`PlatformEvent::as_custom`].
pub trait CustomPlatformEvent: std::any::Any + std::fmt::Debug {
    /// Stable, human-readable identifier used for diagnostics / tracing.
    fn name(&self) -> &'static str;
    /// Borrow as `&dyn Any` so the dispatcher can downcast without leaking
    /// the concrete type to the engine.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Trait implemented by payload types carried inside an [`AppEvent::Custom`].
/// Capability crates use this to inject their own engine-internal event types
/// (e.g. clipboard write requests, forwarded paste requests) without forcing
/// the engine to know about them.
///
/// Implementors also expose [`Any`](std::any::Any) for downcasting so
/// consumers can recover the concrete payload type via
/// [`AppEvent::as_custom`].
pub trait CustomAppEvent: std::any::Any + std::fmt::Debug {
    /// Stable, human-readable identifier used for diagnostics / tracing.
    fn name(&self) -> &'static str;
    /// Borrow as `&dyn Any` so the dispatcher can downcast without leaking
    /// the concrete type to the engine.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Input events originating from the platform / embedder (window system,
/// browser DOM). Pushed into the engine via
/// [`TurApp::push_platform_event`](crate::TurApp::push_platform_event) and
/// dispatched to handlers via [`AppHandler::handle_platform_event`].
///
/// The platform supplies only **raw primitives**: pointer down/move/up/cancel
/// (mouse or touch), device wheel, key, ime, resize. **Gestures**
/// (click, drag, double-click, context-menu, derived scroll, …) are *not*
/// platform events — they are computed inside the engine by the gesture
/// arena from [`PointerInput`] and delivered directly to elements as
/// `ComposedGestureEvent` via `on_gesture_event`. Derived scrolling is routed
/// on the internal bus as [`AppEvent::Scroll`] (never faked as a
/// `PlatformEvent::Wheel`).
///
/// Domain-specific platform events (e.g. clipboard paste from the embedder)
/// travel inside [`PlatformEvent::Custom`] as [`CustomPlatformEvent`]
/// payloads, keeping the engine free of per-domain variant knowledge.
pub enum PlatformEvent {
    Resize {
        logical_width: u32,
        logical_height: u32,
        dpr: f64,
    },
    /// Raw pointer input (mouse or touch). Consumed by the gesture arena to
    /// produce `ComposedGestureEvent`s.
    Pointer(PointerInput),
    /// Device wheel / trackpad scroll from the platform. A touch drag that
    /// the arena resolves to scroll does NOT use this — it is routed through
    /// [`AppEvent::Scroll`] so the wheel pipeline can process real and
    /// derived scroll uniformly.
    Wheel {
        delta_x: f64,
        delta_y: f64,
        position: Offset,
    },
    Key(KeyEvent),
    Ime(ImeEvent),
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

/// Raw pointer primitives supplied by the platform. The gesture arena turns
/// these into higher-level gestures (`ComposedGestureEvent`) and delivers
/// them to elements. There is intentionally no `ContextMenu` variant here —
/// context-menu is a *gesture* derived from a right-button `PointerUp`, not a
/// platform event.
pub enum PointerInput {
    PointerDown {
        position: Offset,
        button: MouseButton,
        time_ms: u64,
        device: PointerDeviceKind,
    },
    PointerUp {
        position: Offset,
        button: MouseButton,
        device: PointerDeviceKind,
    },
    PointerMove {
        position: Offset,
        device: PointerDeviceKind,
    },
    /// The platform cancelled an in-progress pointer sequence (e.g.
    /// `touchcancel` from the browser). The arena releases any captured drag
    /// without firing a click.
    PointerCancel {
        device: PointerDeviceKind,
    },
}

/// Engine-internal event bus. Carries requests produced by elements and
/// handlers during a flush (programmatic scrolls, domain-specific requests
/// such as clipboard writes / paste forwarding) and consumed by other
/// handlers via [`AppHandler::handle_app_event`] within the same fixed-point
/// flush loop. These never cross the embedder boundary.
///
/// Paint requests do **not** live here — they set the `need_paint` flag
/// directly (see [`TurJsContext::need_paint`](crate::core::bridge::TurJsContext)).
///
/// This is also where **derived** scrolling lives: when the gesture arena
/// resolves a touch drag to scroll it emits [`AppEvent::Scroll`] here (rather
/// than faking a `PlatformEvent::Wheel`), so the wheel handler can process
/// real and derived scroll through one path.
///
/// Domain-specific app events (e.g. clipboard write / paste) travel inside
/// [`AppEvent::Custom`] as [`CustomAppEvent`] payloads, keeping the engine
/// free of per-domain variant knowledge.
pub enum AppEvent {
    /// Programmatic scroll request — set the absolute scroll offset of the
    /// target scroll-view node. Emitted by scrollbar drag (where the gesture
    /// handler can't mutate the tree directly due to an active borrow).
    ScrollTo {
        node_id: ElementNodeId,
        offset: f64,
    },
    /// Scroll overflow bubbling — a scroll view consumed as much delta as it
    /// could and is forwarding the remainder (`delta`) to its nearest
    /// scrollable ancestor. Resolved by `ScrollSubsystem`.
    ScrollOverscroll {
        source_id: ElementNodeId,
        delta: f64,
    },
    /// Derived scroll delta produced by the gesture arena (e.g. a touch drag
    /// that the arena resolved to scroll). Consumed by the wheel handler and
    /// routed through the exact same pipeline as a real
    /// [`PlatformEvent::Wheel`].
    Scroll {
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

#[derive(Clone, Debug)]
pub enum ImeEvent {
    CompositionStart,
    CompositionUpdate {
        text: String,
        cursor: Option<(usize, usize)>,
    },
    CompositionEnd {
        text: String,
    },
}
