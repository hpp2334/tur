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

pub enum AppEvent {
    Resize {
        logical_width: u32,
        logical_height: u32,
        dpr: f64,
    },
    Gesture(AppGestureEvent),
    Wheel {
        delta_x: f64,
        delta_y: f64,
        position: Offset,
    },
    ScrollOverscroll {
        source_id: ElementNodeId,
        delta: f64,
    },
    /// Programmatic scroll request — set the absolute scroll offset of the
    /// target scroll-view node. Emitted by scrollbar drag (where the gesture
    /// handler can't mutate the tree directly due to an active borrow).
    ScrollTo {
        node_id: ElementNodeId,
        offset: f64,
    },
    Key(AppKeyEvent),
    Ime(AppImeEvent),
    RequestDraw,
    /// Engine → embedder: write `text` to the system clipboard (copy/cut).
    /// The embedder owns the actual clipboard interaction (e.g.
    /// `navigator.clipboard.writeText` in tur-wasm).
    ClipboardWrite {
        text: String,
    },
    /// Embedder → engine: a paste occurred (the user pressed Cmd+V, the
    /// embedder captured the paste event on its hidden input, and is
    /// forwarding the clipboard text). Handled by ClipboardPasteHandler,
    /// which inserts the text into the focused editable.
    ClipboardPaste {
        text: String,
    },
}

pub enum AppGestureEvent {
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
    /// `touchcancel` from the browser). The arena should release any
    /// captured drag without firing a click.
    PointerCancel {
        device: PointerDeviceKind,
    },
    /// Right-click / context-menu request from the host. Carries the canvas
    /// position of the click. Dispatched to the deepest element under the
    /// cursor that has an `onContextMenu` mutation, mirroring how the web
    /// `contextmenu` event works.
    ContextMenu { position: Offset },
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
