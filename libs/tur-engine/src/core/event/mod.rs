pub mod queue;

use crate::core::element::ElementNodeId;
use crate::core::keyboard::AppKeyEvent;
use tur_shared::{MouseButton, Offset};

/// The physical input device that produced a pointer event. Used by the
/// gesture arena to apply different disambiguation rules for touch vs
/// mouse — touch drags go through slop-based arena resolution (scroll
/// vs drag), while mouse events are dispatched immediately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerDeviceKind {
    Mouse,
    Touch,
}

/// Input events originating from the platform / embedder (window system,
/// browser DOM). Pushed into the engine via
/// [`TurApp::push_platform_event`](crate::TurApp::push_platform_event) and
/// dispatched to handlers via [`AppHandler::handle_platform_event`].
///
/// The platform supplies only **raw primitives**: pointer down/move/up/cancel
/// (mouse or touch), device wheel, key, ime, resize, paste. **Gestures**
/// (click, drag, double-click, context-menu, derived scroll, …) are *not*
/// platform events — they are computed inside the engine by the gesture
/// arena from [`PointerInput`] and delivered directly to elements as
/// `ComposedGestureEvent` via `on_gesture_event`. Derived scrolling is routed
/// on the internal bus as [`AppEvent::Scroll`] (never faked as a
/// `PlatformEvent::Wheel`).
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
    Key(AppKeyEvent),
    Ime(AppImeEvent),
    /// Embedder → engine: a paste occurred (the user pressed Cmd+V, the
    /// embedder captured the paste event on its hidden input, and is
    /// forwarding the clipboard text). Handled by `ClipboardPasteHandler`,
    /// which inserts the text into the focused editable.
    ClipboardPaste {
        text: String,
    },
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
/// handlers during a flush (programmatic scrolls, clipboard writes) and
/// consumed by other handlers via [`AppHandler::handle_app_event`] within the
/// same fixed-point flush loop. These never cross the embedder boundary.
///
/// Paint requests do **not** live here — they set the `need_paint` flag
/// directly (see [`TurJsContext::need_paint`](crate::core::bridge::TurJsContext)).
///
/// This is also where **derived** scrolling lives: when the gesture arena
/// resolves a touch drag to scroll it emits [`AppEvent::Scroll`] here (rather
/// than faking a `PlatformEvent::Wheel`), so the wheel handler can process
/// real and derived scroll through one path.
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
    /// scrollable ancestor. Resolved by `ScrollChainingHandler`.
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
    /// Engine → embedder: write `text` to the system clipboard (copy/cut).
    /// Consumed by `ClipboardWriteHandler` (std module), which drives the
    /// injected `Clipboard` capability — the embedder owns the actual
    /// clipboard interaction (e.g. `navigator.clipboard.writeText` in
    /// tur-wasm).
    ClipboardWrite {
        text: String,
    },
}

#[derive(Clone, Debug)]
pub enum AppImeEvent {
    CompositionStart,
    CompositionUpdate {
        text: String,
        cursor: Option<(usize, usize)>,
    },
    CompositionEnd {
        text: String,
    },
}
