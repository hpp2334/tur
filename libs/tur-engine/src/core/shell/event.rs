//! Shell ingress: raw input primitives originating from the embedder
//! (window system, browser DOM) that the engine consumes via the
//! [`PlatformEvent::Shell`](crate::core::platform::PlatformEvent::Shell)
//! envelope.
//!
//! The platform supplies only **raw primitives**: pointer down/move/up/cancel
//! (mouse or touch), device wheel, key, ime, resize. **Gestures**
//! (click, drag, double-click, context-menu, derived scroll, …) are *not*
//! shell events — they are computed inside the engine by the gesture
//! arena from [`PointerInput`] and delivered directly to elements as
//! `ComposedGestureEvent` via `on_gesture_event`. Derived scrolling is routed
//! on the internal bus as [`crate::core::app::AppEvent::Scroll`] (never
//! faked as a [`ShellEvent::Wheel`]).

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

/// Raw shell input events: what the host window system / browser DOM just
/// told the engine. See the [module docs](self) for the full semantics.
pub enum ShellEvent {
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
    /// [`crate::core::app::AppEvent::Scroll`] so the wheel pipeline can
    /// process real and derived scroll uniformly.
    Wheel {
        delta_x: f64,
        delta_y: f64,
        position: Offset,
    },
    Key(KeyEvent),
    Ime(ImeEvent),
}

/// Raw pointer primitives supplied by the platform. The gesture arena turns
/// these into higher-level gestures (`ComposedGestureEvent`) and delivers
/// them to elements. There is intentionally no `ContextMenu` variant here —
/// context-menu is a *gesture* derived from a right-button `PointerUp`, not
/// a shell event.
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
