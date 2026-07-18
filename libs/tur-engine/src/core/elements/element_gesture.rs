use crate::core::layout::{MouseButton, Offset};

use crate::core::element::ElementNodeId;
use crate::core::mutation::{MutationHandle, IntoJsArgs, PendingMutationInvocationQueue};
use crate::core::event::queue::AppEventQueue;
use crate::core::event::{AppEvent, PointerDeviceKind};
use crate::core::focus::FocusManager;
use std::cell::Cell;

pub enum ComposedGestureEvent {
    PointerDown { local: Offset, global: Offset, button: MouseButton, device: PointerDeviceKind },
    /// Double-click — computed from two `PointerDown`s within ≤500 ms and
    /// ≤5 px. Used by EditableText for word selection.
    PointerDoubleDown { local: Offset, global: Offset, button: MouseButton },
    /// Triple-click — computed from three consecutive clicks in the same
    /// time/position window. Used by EditableText for line selection.
    PointerTripleDown { local: Offset, global: Offset, button: MouseButton },
    PointerMove { local: Offset, global: Offset, device: PointerDeviceKind },
    PointerUp { local: Offset, global: Offset, button: MouseButton, device: PointerDeviceKind },
    /// Click — dispatched after a `PointerUp` that landed on the same
    /// element as the `PointerDown`. `PointerInteractElement` handles this
    /// by invoking its `on_click` mutation. Dispatch stops at the first
    /// click-opaque element in the hit-path.
    Click { local: Offset, global: Offset },
    /// Right-click. `local` is relative to the element; `global` is canvas-
    /// relative. Dispatched to every element in the hit-path (deepest first)
    /// so layered views can each inspect the event.
    ContextMenu { local: Offset, global: Offset },
}

pub struct ElementOnGestureContext<'a> {
    event_queue: &'a mut AppEventQueue,
    focus_manager: &'a mut FocusManager,
    mutation_queue: &'a mut PendingMutationInvocationQueue,
    need_paint: &'a Cell<bool>,
    node_id: ElementNodeId,
}

impl<'a> ElementOnGestureContext<'a> {
    pub fn new(
        event_queue: &'a mut AppEventQueue,
        focus_manager: &'a mut FocusManager,
        mutation_queue: &'a mut PendingMutationInvocationQueue,
        need_paint: &'a Cell<bool>,
        node_id: ElementNodeId,
    ) -> Self {
        Self {
            event_queue,
            focus_manager,
            mutation_queue,
            need_paint,
            node_id,
        }
    }

    pub fn request_paint(&mut self) {
        self.need_paint.set(true);
    }

    /// Request that the scroll-view node be scrolled to an absolute offset.
    /// Resolved post-dispatch by the `ScrollToHandler` (the tree is mutably
    /// borrowed for the duration of a gesture event, so we defer).
    pub fn request_scroll_to(&mut self, node_id: ElementNodeId, offset: f64) {
        self.event_queue
            .push(AppEvent::ScrollTo { node_id, offset });
    }

    pub fn request_focus(&mut self, id: ElementNodeId) {
        self.focus_manager.set_focus(id);
    }

    pub fn request_own_focus(&mut self) {
        self.focus_manager.set_focus(self.node_id);
    }

    pub fn push_event<E: IntoJsArgs>(&mut self, mutation: MutationHandle<E>, event: E) {
        self.mutation_queue.push(mutation, event);
    }
}

/// Trait for elements that handle gesture events (pointer down/move/up,
/// click, context menu).
///
/// `on_gesture_event` returns `bool`: `true` means the element claims the
/// gesture (the arena will route subsequent move/up events to it); `false`
/// means the element is not interested (the arena falls through to the next
/// candidate or to scroll). The return value is only meaningful for
/// `PointerDown` — other events are only dispatched to the already-captured
/// path. Default: `true` (claim).
pub trait ElementOnGesture: 'static {
    fn on_gesture_event(
        &mut self,
        cx: &mut ElementOnGestureContext,
        event: &ComposedGestureEvent,
    ) -> bool {
        let _ = cx;
        let _ = event;
        true
    }
}
