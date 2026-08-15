//! Platform-side events: raw input primitives originating from the embedder
//! (window system, browser DOM).
//!
//! Pushed into the engine via
//! [`TurApp::push_platform_event`](crate::TurApp::push_platform_event) and
//! dispatched to handlers via `Subsystem::handle_platform_event`.
//!
//! Every platform event is a [`PlatformEvent::Shell`] — an event delivered to
//! one **shell** (host surface / view root), carrying its
//! [`ViewRootId`](crate::core::element::ViewRootId) plus a
//! [`ShellEventPayload``]. The platform supplies only **raw primitives**:
//! pointer down/move/up/cancel (mouse or touch), device wheel, key, ime,
//! resize. **Gestures** (click, drag, double-click, context-menu, derived
//! scroll, …) are *not* platform events — they are computed inside the
//! engine by the gesture arena from [`PointerInput`] and delivered directly
//! to elements as `ComposedGestureEvent` via `on_gesture_event`. Derived
//! scrolling is routed on the internal bus as
//! [`crate::core::app::AppEvent::Scroll`] (never faked as a
//! [`ShellEventPayload::Wheel`]).
//!
//! ## Root-routing contract
//!
//! - **Pointer / Wheel / Resize** — inherently tied to one surface. The
//!   engine routes them to `view_root_id`'s element tree; positions are in
//!   that root's local (canvas) coordinate space.
//! - **Key / Ime / Custom** — dispatched to the focused element / internal
//!   handlers **without root gating**. Element focus is instance-global (a
//!   single focus scope per engine instance). Hosts are responsible for
//!   shell focus: only push Key/Ime/Custom for shells that hold OS focus,
//!   stamping that shell's root id (the field is informational for these
//!   payload kinds — forward-compatible with per-root focus).
//!
//! Domain-specific platform events (e.g. clipboard paste from the embedder)
//! travel inside [`ShellEventPayload::Custom`] as
//! [`CustomShellEventPayload`] payloads, keeping the engine free of
//! per-domain variant knowledge.

use crate::core::element::ViewRootId;
use crate::core::layout::{MouseButton, Offset};
use crate::core::platform::key_event::KeyEvent;

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
/// [`ShellEventPayload::Custom`]. Capability crates use this to inject their
/// own platform-originated event types (e.g. clipboard paste from the
/// embedder) without forcing the engine to know about them.
///
/// Implementors also expose [`Any`](std::any::Any) for downcasting so
/// consumers can recover the concrete payload type via
/// [`PlatformEvent::as_custom`].
///
/// `Send + Sync` is required so a `PlatformEvent` can cross the worker↔main
/// channel boundary (Phase 4+). All current implementors are plain data
/// (`{ text: String }`, etc.); the bound is a forward-looking guard.
pub trait CustomShellEventPayload: std::any::Any + std::fmt::Debug + Send + Sync {
    /// Stable, human-readable identifier used for diagnostics / tracing.
    fn name(&self) -> &'static str;
    /// Borrow as `&dyn Any` so the dispatcher can downcast without leaking
    /// the concrete type to the engine.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Input events originating from the platform / embedder (window system,
/// browser DOM). See the [module docs](self) for the full semantics.
///
/// A single event delivered to one shell (host surface / view root). The
/// `Shell` wrapper leaves room for future device-level event kinds that are
/// not tied to a single shell (e.g. battery / connectivity notifications).
pub enum PlatformEvent {
    Shell(ShellEvent),
}

/// The shell-targeted event: everything the embedder delivers to one host
/// surface (canvas / window), stamped with the view root it targets.
pub struct ShellEvent {
    /// The view root (host surface) this event targets.
    pub view_root_id: ViewRootId,
    /// The raw input primitive.
    pub payload: ShellEventPayload,
}

/// Raw primitives delivered to a shell. See the [module docs](self) for the
/// root-routing contract per kind.
pub enum ShellEventPayload {
    /// The host surface was resized (logical pixels + device pixel ratio).
    Resize {
        logical_width: u32,
        logical_height: u32,
        dpr: f64,
    },
    /// Raw pointer input (mouse or touch). Consumed by the gesture arena to
    /// produce `ComposedGestureEvent`s. The position is in the target
    /// root's local (canvas) coordinate space.
    Pointer { input: PointerInput },
    /// Device wheel / trackpad scroll (position in the target root's local
    /// space). A touch drag that the arena resolves to scroll does NOT use
    /// this — it is routed through [`crate::core::app::AppEvent::Scroll`]
    /// so the wheel pipeline can process real and derived scroll uniformly.
    Wheel {
        delta_x: f64,
        delta_y: f64,
        position: Offset,
    },
    /// A key event for the focused element. Routed via the instance-global
    /// focus scope — the engine does not gate it on `view_root_id` (hosts
    /// only push Key for focused shells).
    Key(KeyEvent),
    /// An IME composition event for the focused element. Routed like
    /// [`ShellEventPayload::Key`].
    Ime(ImeEvent),
    /// Domain-specific platform event (e.g. clipboard paste from the
    /// embedder). Capability crates define their own payload types
    /// implementing [`CustomShellEventPayload`]; consumers downcast via
    /// [`PlatformEvent::as_custom`].
    Custom(Box<dyn CustomShellEventPayload>),
}

impl PlatformEvent {
    /// Build a shell-targeted event.
    pub fn shell(view_root_id: ViewRootId, payload: ShellEventPayload) -> Self {
        Self::Shell(ShellEvent {
            view_root_id,
            payload,
        })
    }

    /// The view root (host surface) this event targets. For Key / Ime /
    /// Custom payloads this is informational — the engine does not root-gate
    /// those kinds (see the [module docs](self)).
    pub fn view_root_id(&self) -> ViewRootId {
        match self {
            Self::Shell(e) => e.view_root_id,
        }
    }

    /// The raw input primitive.
    pub fn payload(&self) -> &ShellEventPayload {
        match self {
            Self::Shell(e) => &e.payload,
        }
    }

    /// If this event carries a [`ShellEventPayload::Custom`] payload of
    /// type `T`, borrow the payload; otherwise `None`.
    pub fn as_custom<T: CustomShellEventPayload>(&self) -> Option<&T> {
        if let ShellEventPayload::Custom(p) = self.payload() {
            p.as_any().downcast_ref::<T>()
        } else {
            None
        }
    }
}

/// Raw pointer primitives supplied by the platform. The gesture arena turns
/// these into higher-level gestures (`ComposedGestureEvent`) and delivers
/// them to elements. There is intentionally no `ContextMenu` variant here —
/// context-menu is a *gesture* derived from a right-button `PointerUp`, not
/// a platform event.
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
        time_ms: u64,
    },
    PointerMove {
        position: Offset,
        device: PointerDeviceKind,
        time_ms: u64,
    },
    /// The platform cancelled an in-progress pointer sequence (e.g.
    /// `touchcancel` from the browser). The arena releases any captured drag
    /// without firing a click.
    PointerCancel { device: PointerDeviceKind },
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
