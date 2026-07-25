mod arena;
mod composer;

use crate::core::elements::{ComposedGestureEvent, ElementOnGestureContext};
use crate::core::app::AppEvent;
use crate::core::platform::{PlatformEvent, PointerDeviceKind, PointerInput};
use crate::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use crate::core::focus::helper::find_focusable_in_path;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};
use crate::core::hit_test::HitTest;
use crate::builtin_plugins::gesture::pointer_interact::PointerInteractElement;
use crate::builtin_plugins::scroll::event::push_fling;
use crate::core::elements::NodeTreeData;
use crate::core::layout::{MouseButton, Offset};

pub use composer::ClickKind;

use arena::{ArenaWinnerKind, GestureArena, TouchCancelOutcome, TouchMoveOutcome, TouchUpOutcome};
use composer::GestureEventComposer;

/// Debug label for a `TouchUpOutcome` (the enum doesn't derive `Debug`).
fn outcome_kind(o: &TouchUpOutcome) -> &'static str {
    match o {
        TouchUpOutcome::DragEnded => "DragEnded",
        TouchUpOutcome::ScrollEnded { .. } => "ScrollEnded",
        TouchUpOutcome::Tap { .. } => "Tap",
        TouchUpOutcome::Idle => "Idle",
    }
}

pub struct GestureSubsystem {
    arena: GestureArena,
    composer: GestureEventComposer,
}

impl GestureSubsystem {
    pub fn new() -> Self {
        Self {
            arena: GestureArena::new(),
            composer: GestureEventComposer::new(),
        }
    }
}

impl Default for GestureSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl Subsystem for GestureSubsystem {
    fn handle_platform_event(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        event: &PlatformEvent,
    ) {
        match event {
            PlatformEvent::Pointer(PointerInput::PointerDown {
                position,
                button,
                time_ms,
                device,
            }) => match device {
                PointerDeviceKind::Mouse => {
                    self.handle_mouse_pointer_down(cx, *position, *button, *time_ms);
                }
                PointerDeviceKind::Touch => {
                    self.handle_touch_pointer_down(cx, *position, *time_ms);
                }
            },
            PlatformEvent::Pointer(PointerInput::PointerMove { position, device, time_ms }) => match device {
                PointerDeviceKind::Mouse => {
                    self.handle_mouse_pointer_move(cx, *position);
                }
                PointerDeviceKind::Touch => {
                    self.handle_touch_pointer_move(cx, *position, *time_ms);
                }
            },
            PlatformEvent::Pointer(PointerInput::PointerUp {
                position,
                button,
                device,
                time_ms,
            }) => match device {
                PointerDeviceKind::Mouse => {
                    self.handle_mouse_pointer_up(cx, *position, *button);
                }
                PointerDeviceKind::Touch => {
                    self.handle_touch_pointer_up(cx, *position, *time_ms);
                }
            },
            PlatformEvent::Pointer(PointerInput::PointerCancel { device }) => match device {
                PointerDeviceKind::Mouse => {
                    self.handle_mouse_pointer_cancel(cx);
                }
                PointerDeviceKind::Touch => {
                    self.handle_touch_pointer_cancel(cx);
                }
            },
            _ => {}
        }
    }
}

// ── Mouse path (immediate dispatch, no arena) ────────────────────────

impl GestureSubsystem {
    fn handle_mouse_pointer_down(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        position: Offset,
        button: MouseButton,
        time_ms: u64,
    ) {
        let path = HitTest::new(&cx.element_tree.borrow()).path(position);
        let target = path.first().copied();
        self.composer.on_pointer_down(target, path.clone());

        let kind = self.composer.classify_click(position, time_ms);
        for id in &path {
            let local = local_position(cx, *id, position);
            let event = match kind {
                ClickKind::Single => {
                    ComposedGestureEvent::PointerDown { local, global: position, button, device: PointerDeviceKind::Mouse }
                }
                ClickKind::Double => {
                    ComposedGestureEvent::PointerDoubleDown { local, global: position, button }
                }
                ClickKind::Triple => {
                    ComposedGestureEvent::PointerTripleDown { local, global: position, button }
                }
            };
            dispatch_gesture_event(cx, *id, &event);
        }
    }

    fn handle_mouse_pointer_move(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        position: Offset,
    ) {
        if !self.composer.is_tracking_drag() {
            return;
        }

        let path: Vec<ElementNodeId> = self.composer.pointer_down_path().to_vec();
        for id in &path {
            let local = local_position(cx, *id, position);
            dispatch_gesture_event(
                cx,
                *id,
                &ComposedGestureEvent::PointerMove { local, global: position, device: PointerDeviceKind::Mouse },
            );
        }
    }

    fn handle_mouse_pointer_up(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        position: Offset,
        button: MouseButton,
    ) {
        let down_target = self.composer.pointer_down_target();
        let down_path: Vec<ElementNodeId> = self.composer.pointer_down_path().to_vec();

        for id in &down_path {
            let local = local_position(cx, *id, position);
            dispatch_gesture_event(
                cx,
                *id,
                &ComposedGestureEvent::PointerUp { local, global: position, button, device: PointerDeviceKind::Mouse },
            );
        }

        // Text-edit-focus: clear focus when clicking outside any focusable
        // element (unless the pointer-down target was the focused element —
        // the element's own handler will manage focus in that case).
        let (focusable_id, hit_path) = {
            let tree = cx.element_tree.borrow();
            let hit_path = HitTest::new(&tree).path(position);
            let focusable_id = find_focusable_in_path(&tree, &hit_path);
            (focusable_id, hit_path)
        };
        let _ = hit_path;
        // Hoist the `borrow()` out of the `let`-chain below: a `borrow()`
        // temporary created inside a `let`-chain condition has its lifetime
        // extended through the entire `if` block (temporary lifetime
        // extension), which would keep the immutable borrow alive across the
        // `borrow_mut()` and panic with "RefCell already borrowed" whenever a
        // pointer-up lands outside any focusable while an element is focused
        // (e.g. tapping a `PointerInteract` button in the github-viewer case).
        let focused = cx.focus_manager.borrow().focused();
        if focusable_id.is_none()
            && focused.is_some()
            && down_target != focused
        {
            cx.focus_manager.borrow_mut().clear_focus();
        }

        let click_eligible = match down_target {
            Some(id) => HitTest::new(&cx.element_tree.borrow()).contains(position, id),
            None => false,
        };

        let resolved = self.composer.on_pointer_up(click_eligible);
        if resolved {
            // Derive the gesture from the button that was released: a primary
            // (left) release becomes a `Click`; a secondary (right) release
            // becomes a `ContextMenu`. Context-menu is a *gesture*, not a
            // platform event, so it is produced here rather than carried in
            // from the embedder.
            match button {
                MouseButton::Left => dispatch_click(cx, position),
                MouseButton::Right => dispatch_context_menu(cx, position),
                _ => {}
            }
        }
    }

    fn handle_mouse_pointer_cancel(&mut self, cx: &mut SubsystemFlushContext<'_>) {
        let down_path: Vec<ElementNodeId> = self.composer.pointer_down_path().to_vec();
        for id in &down_path {
            let local = local_position(cx, *id, Offset::new(0.0, 0.0));
            dispatch_gesture_event(
                cx,
                *id,
                &ComposedGestureEvent::PointerUp {
                    local,
                    global: Offset::new(0.0, 0.0),
                    button: MouseButton::Left,
                    device: PointerDeviceKind::Mouse,
                },
            );
        }
        self.composer.on_pointer_up(false);
    }
}

// ── Touch path (arena-based) ──────────────────────────────────────────

impl GestureSubsystem {
    fn handle_touch_pointer_down(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        position: Offset,
        time_ms: u64,
    ) {
        let path = HitTest::new(&cx.element_tree.borrow()).path(position);
        tracing::info!(
            "TOUCH DOWN at ({},{}) t={time_ms} path_len={}",
            position.x, position.y, path.len()
        );
        self.arena.on_touch_down(position, time_ms, path);
    }

    fn handle_touch_pointer_move(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        position: Offset,
        now_ms: u64,
    ) {
        let outcome = self.arena.on_touch_move(position, now_ms);
        match outcome {
            TouchMoveOutcome::Idle => {}
            TouchMoveOutcome::SlopExceeded => {
                let hit_path: Vec<ElementNodeId> =
                    self.arena.touch_hit_path().unwrap_or(&[]).to_vec();
                let down_position = self
                    .arena
                    .touch_down_position()
                    .unwrap_or(position);

                // Probe gesture elements: dispatch PointerDown { device: Touch }
                // to each until one claims (returns true).
                let mut drag_winner = None;
                for &id in &hit_path {
                    let has_gesture = cx
                        .element_tree
                        .borrow()
                        .get_element(id)
                        .and_then(|n| n.element.as_ref())
                        .map(|e| e.has_on_gesture())
                        .unwrap_or(false);
                    if !has_gesture {
                        continue;
                    }
                    let local = local_position(cx, id, down_position);
                    let claimed = dispatch_gesture_event(
                        cx,
                        id,
                        &ComposedGestureEvent::PointerDown {
                            local,
                            global: down_position,
                            button: MouseButton::Left,
                            device: PointerDeviceKind::Touch,
                        },
                    );
                    if claimed {
                        drag_winner = Some(id);
                        break;
                    }
                }

                if let Some(target) = drag_winner {
                    // Drag won — set up capture and dispatch PointerMove.
                    tracing::info!("SLOP: drag won target={target:?}");
                    self.arena.resolve(ArenaWinnerKind::Drag);
                    self.composer
                        .on_pointer_down(Some(target), hit_path.clone());
                    for &id in &hit_path {
                        let local = local_position(cx, id, position);
                        dispatch_gesture_event(
                            cx,
                            id,
                            &ComposedGestureEvent::PointerMove {
                                local,
                                global: position,
                                device: PointerDeviceKind::Touch,
                            },
                        );
                    }
                } else {
                    // No drag element claimed — resolve to scroll.
                    tracing::info!("SLOP: no drag claim -> SCROLL");
                    self.arena.resolve(ArenaWinnerKind::Scroll);
                    // Emit the derived scroll delta on the internal bus (not a
                    // fake platform wheel) so the wheel handler processes real
                    // and derived scroll uniformly.
                    let dx = position.x - down_position.x;
                    let dy = position.y - down_position.y;
                    cx.app_event_queue.push(AppEvent::Scroll {
                        delta_x: -dx,
                        delta_y: -dy,
                        position,
                    });
                }
            }
            TouchMoveOutcome::DragMoved => {
                let path: Vec<ElementNodeId> = self.composer.pointer_down_path().to_vec();
                for id in &path {
                    let local = local_position(cx, *id, position);
                    dispatch_gesture_event(
                        cx,
                        *id,
                        &ComposedGestureEvent::PointerMove {
                            local,
                            global: position,
                            device: PointerDeviceKind::Touch,
                        },
                    );
                }
            }
            TouchMoveOutcome::Scroll {
                delta_x,
                delta_y,
                position,
            } => {
                cx.app_event_queue.push(AppEvent::Scroll {
                    delta_x,
                    delta_y,
                    position,
                });
            }
        }
    }

    fn handle_touch_pointer_up(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        position: Offset,
        time_ms: u64,
    ) {
        let outcome = self.arena.on_touch_up(position, time_ms);
        tracing::info!(
            "TOUCH UP at ({},{}) t={time_ms} outcome={:?}",
            position.x, position.y, outcome_kind(&outcome)
        );
        match outcome {
            TouchUpOutcome::DragEnded => {
                let path: Vec<ElementNodeId> = self.composer.pointer_down_path().to_vec();
                for id in &path {
                    let local = local_position(cx, *id, position);
                    dispatch_gesture_event(
                        cx,
                        *id,
                        &ComposedGestureEvent::PointerUp {
                            local,
                            global: position,
                            button: MouseButton::Left,
                            device: PointerDeviceKind::Touch,
                        },
                    );
                }
                self.composer.on_pointer_up(false);
            }
            TouchUpOutcome::ScrollEnded { vx, vy } => {
                // Seed a kinetic-scroll fling from the drag's final velocity.
                // Scroll delta is the negation of touch-movement direction
                // (touch moves up → content scrolls down), matching the
                // convention used by `TouchMoveOutcome::Scroll`.
                push_fling(cx.app_event_queue, -vx, -vy, position);
            }
            TouchUpOutcome::Tap { position, time_ms } => {
                // The touch sequence ended as a tap (short + sub-slop, no
                // drag/scroll winner). The engine synthesizes the click on
                // every host — drive the mouse down→up path in-process
                // (composer capture + click classification + dispatch) so the
                // tap behaves exactly like a mouse click, without faking a
                // platform pointer event or touching `PointerSubsystem` (no
                // hover on touch). Embedders must NOT also forward a
                // host-synthesized click for the same tap (double dispatch).
                self.handle_mouse_pointer_down(cx, position, MouseButton::Left, time_ms);
                self.handle_mouse_pointer_up(cx, position, MouseButton::Left);
            }
            TouchUpOutcome::Idle => {}
        }
    }

    fn handle_touch_pointer_cancel(&mut self, cx: &mut SubsystemFlushContext<'_>) {
        let outcome = self.arena.on_touch_cancel();
        match outcome {
            TouchCancelOutcome::DragCanceled => {
                let path: Vec<ElementNodeId> = self.composer.pointer_down_path().to_vec();
                for id in &path {
                    let local = local_position(cx, *id, Offset::new(0.0, 0.0));
                    dispatch_gesture_event(
                        cx,
                        *id,
                        &ComposedGestureEvent::PointerUp {
                            local,
                            global: Offset::new(0.0, 0.0),
                            button: MouseButton::Left,
                            device: PointerDeviceKind::Touch,
                        },
                    );
                }
                self.composer.on_pointer_up(false);
            }
            TouchCancelOutcome::Idle => {}
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Dispatch `ComposedGestureEvent::Click` to every `PointerInteractElement`
/// in the hit-path, stopping at the first click-opaque element.
fn dispatch_click(cx: &mut SubsystemFlushContext<'_>, position: Offset) {
    let hit_path = HitTest::new(&cx.element_tree.borrow()).path(position);
    for node_id in &hit_path {
        let local = local_position(cx, *node_id, position);
        dispatch_gesture_event(
            cx,
            *node_id,
            &ComposedGestureEvent::Click { local, global: position },
        );
        if is_click_opaque(&cx.element_tree.borrow(), *node_id) {
            break;
        }
    }
}

/// Dispatch `ComposedGestureEvent::ContextMenu` to every element in the
/// hit-path (mirrors how the web `contextmenu` event bubbles).
fn dispatch_context_menu(cx: &mut SubsystemFlushContext<'_>, position: Offset) {
    let hit_path = HitTest::new(&cx.element_tree.borrow()).path(position);
    for node_id in &hit_path {
        let local = local_position(cx, *node_id, position);
        dispatch_gesture_event(
            cx,
            *node_id,
            &ComposedGestureEvent::ContextMenu { local, global: position },
        );
    }
}

fn is_click_opaque(tree: &NodeTreeData, id: ElementNodeId) -> bool {
    tree.get_element(id)
        .and_then(|node| node.element.as_ref())
        .map(|e| {
            e.cast::<PointerInteractElement>()
                .map(|p| p.is_click_opaque())
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn local_position(
    cx: &SubsystemFlushContext<'_>,
    node_id: ElementNodeId,
    global: Offset,
) -> Offset {
    let mut abs_x = 0.0f64;
    let mut abs_y = 0.0f64;
    let tree = cx.element_tree.borrow();
    let mut current: Option<NodeId> = Some(node_id.into());
    while let Some(cid) = current {
        if let Some(n) = tree.get_element(ElementNodeId::new(cid.as_u64())) {
            abs_x += n.computed_layout.offset.x;
            abs_y += n.computed_layout.offset.y;
            current = n.parent;
        } else if let Some(f) = tree.get_fragment(FragmentNodeId::new(cid.as_u64())) {
            current = Some(f.parent);
        } else {
            break;
        }
    }
    Offset::new(global.x - abs_x, global.y - abs_y)
}

/// Dispatch a gesture event to an element. Returns `true` if the element
/// claimed the gesture (only meaningful for `PointerDown`). Only marks the
/// element dirty if claimed.
fn dispatch_gesture_event(
    cx: &mut SubsystemFlushContext<'_>,
    id: ElementNodeId,
    event: &ComposedGestureEvent,
) -> bool {
    let mut tree = cx.element_tree.borrow_mut();
    let Some(node) = tree.get_element_mut(id) else {
        return false;
    };
    let Some(ref mut element) = node.element else {
        return false;
    };
    let mut fm = cx.focus_manager.borrow_mut();
    let mut mq = cx.mutation_queue.borrow_mut();
    let mut el_cx = ElementOnGestureContext::new(
        &mut *cx.app_event_queue,
        &mut fm,
        &mut mq,
        cx.need_paint,
        id,
    );
    let claimed = element.on_gesture_event(&mut el_cx, event);
    if claimed {
        tree.mark_dirty(id.into());
    }
    claimed
}
