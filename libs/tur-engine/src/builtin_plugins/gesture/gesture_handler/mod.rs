mod arena;
mod composer;

use crate::builtin_plugins::gesture::pointer_interact::PointerInteractElement;
use crate::builtin_plugins::scroll::event::push_fling;
use crate::core::app::AppEvent;
use crate::core::element::{ElementNodeId, NodeId, ViewRootId};
use crate::core::elements::NodeTree;
use crate::core::elements::NodeTreeData;
use crate::core::elements::{ComposedGestureEvent, ElementOnGestureContext};
use crate::core::focus::helper::find_focusable_in_path;
use crate::core::hit_test::HitTest;
use crate::core::layout::{MouseButton, Offset};
use crate::core::platform::{PlatformEvent, PointerDeviceKind, PointerInput, ShellEventPayload};
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

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
    fn handle_platform_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &PlatformEvent) {
        // Pointer events are routed to one view root — resolve its tree and
        // thread it through every handler (positions are root-local).
        let ShellEventPayload::Pointer { input } = event.payload() else {
            return;
        };
        let root = event.view_root_id();
        let Some(tree) = cx.tree_of_root(root) else {
            return;
        };
        match input {
            PointerInput::PointerDown {
                position,
                button,
                time_ms,
                device,
            } => match device {
                PointerDeviceKind::Mouse => {
                    self.handle_mouse_pointer_down(
                        cx, &tree, *position, *button, *time_ms, *device,
                    );
                }
                PointerDeviceKind::Touch => {
                    self.handle_touch_pointer_down(cx, &tree, *position, *time_ms);
                }
            },
            PointerInput::PointerMove {
                position,
                device,
                time_ms,
            } => match device {
                PointerDeviceKind::Mouse => {
                    self.handle_mouse_pointer_move(cx, &tree, *position, *device);
                }
                PointerDeviceKind::Touch => {
                    self.handle_touch_pointer_move(cx, &tree, root, *position, *time_ms);
                }
            },
            PointerInput::PointerUp {
                position,
                button,
                device,
                time_ms,
            } => match device {
                PointerDeviceKind::Mouse => {
                    self.handle_mouse_pointer_up(cx, &tree, *position, *button, *device);
                }
                PointerDeviceKind::Touch => {
                    self.handle_touch_pointer_up(cx, &tree, root, *position, *time_ms);
                }
            },
            PointerInput::PointerCancel { device } => match device {
                PointerDeviceKind::Mouse => {
                    self.handle_mouse_pointer_cancel(cx, &tree, *device);
                }
                PointerDeviceKind::Touch => {
                    self.handle_touch_pointer_cancel(cx, &tree);
                }
            },
        }
    }
}

// ── Mouse path (immediate dispatch, no arena) ────────────────────────

impl GestureSubsystem {
    fn handle_mouse_pointer_down(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        tree: &NodeTree,
        position: Offset,
        button: MouseButton,
        time_ms: u64,
        device: PointerDeviceKind,
    ) {
        let path = HitTest::new(&tree.borrow()).path(position);
        let target = path.first().copied();
        self.composer.on_pointer_down(target, path.clone());

        let kind = self.composer.classify_click(position, time_ms);
        for id in &path {
            let local = local_position(tree, *id, position);
            let event = match kind {
                ClickKind::Single => ComposedGestureEvent::PointerDown {
                    local,
                    global: position,
                    button,
                    device,
                },
                ClickKind::Double => ComposedGestureEvent::PointerDoubleDown {
                    local,
                    global: position,
                    button,
                    device,
                },
                ClickKind::Triple => ComposedGestureEvent::PointerTripleDown {
                    local,
                    global: position,
                    button,
                    device,
                },
            };
            dispatch_gesture_event(cx, *id, &event);
        }
    }

    fn handle_mouse_pointer_move(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        tree: &NodeTree,
        position: Offset,
        device: PointerDeviceKind,
    ) {
        if !self.composer.is_tracking_drag() {
            return;
        }

        let path: Vec<ElementNodeId> = self.composer.pointer_down_path().to_vec();
        for id in &path {
            let local = local_position(tree, *id, position);
            dispatch_gesture_event(
                cx,
                *id,
                &ComposedGestureEvent::PointerMove {
                    local,
                    global: position,
                    device,
                },
            );
        }
    }

    fn handle_mouse_pointer_up(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        tree: &NodeTree,
        position: Offset,
        button: MouseButton,
        device: PointerDeviceKind,
    ) {
        let down_target = self.composer.pointer_down_target();
        let down_path: Vec<ElementNodeId> = self.composer.pointer_down_path().to_vec();

        for id in &down_path {
            let local = local_position(tree, *id, position);
            dispatch_gesture_event(
                cx,
                *id,
                &ComposedGestureEvent::PointerUp {
                    local,
                    global: position,
                    button,
                    device,
                },
            );
        }

        // Text-edit-focus: clear focus when clicking outside any focusable
        // element (unless the pointer-down target was the focused element —
        // the element's own handler will manage focus in that case).
        let focusable_id = {
            let t = tree.borrow();
            let hit_path = HitTest::new(&t).path(position);
            find_focusable_in_path(&t, &hit_path)
        };
        // Hoist the `borrow()` out of the `let`-chain below: a `borrow()`
        // temporary created inside a `let`-chain condition has its lifetime
        // extended through the entire `if` block (temporary lifetime
        // extension), which would keep the immutable borrow alive across the
        // `borrow_mut()` and panic with "RefCell already borrowed" whenever a
        // pointer-up lands outside any focusable while an element is focused
        // (e.g. tapping a `PointerInteract` button in the github-viewer case).
        let focused = cx.focus_manager.borrow().focused();
        if focusable_id.is_none() && focused.is_some() && down_target != focused {
            cx.focus_manager.borrow_mut().clear_focus();
        }

        let click_eligible = match down_target {
            Some(id) => HitTest::new(&tree.borrow()).contains(position, id),
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
                MouseButton::Left => dispatch_click(cx, tree, position, device),
                MouseButton::Right => dispatch_context_menu(cx, tree, position, device),
                _ => {}
            }
        }
    }

    fn handle_mouse_pointer_cancel(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        tree: &NodeTree,
        device: PointerDeviceKind,
    ) {
        let down_path: Vec<ElementNodeId> = self.composer.pointer_down_path().to_vec();
        for id in &down_path {
            let local = local_position(tree, *id, Offset::new(0.0, 0.0));
            dispatch_gesture_event(
                cx,
                *id,
                &ComposedGestureEvent::PointerUp {
                    local,
                    global: Offset::new(0.0, 0.0),
                    button: MouseButton::Left,
                    device,
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
        _cx: &mut SubsystemFlushContext<'_>,
        tree: &NodeTree,
        position: Offset,
        time_ms: u64,
    ) {
        let path = HitTest::new(&tree.borrow()).path(position);
        tracing::info!(
            "TOUCH DOWN at ({},{}) t={time_ms} path_len={}",
            position.x,
            position.y,
            path.len()
        );
        self.arena.on_touch_down(position, time_ms, path);
    }

    fn handle_touch_pointer_move(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        tree: &NodeTree,
        root: ViewRootId,
        position: Offset,
        now_ms: u64,
    ) {
        let outcome = self.arena.on_touch_move(position, now_ms);
        match outcome {
            TouchMoveOutcome::Idle => {}
            TouchMoveOutcome::SlopExceeded => {
                let hit_path: Vec<ElementNodeId> =
                    self.arena.touch_hit_path().unwrap_or(&[]).to_vec();
                let down_position = self.arena.touch_down_position().unwrap_or(position);

                // Probe gesture elements: find the first that accepts a touch
                // drag claim, then dispatch PointerDown { device: Touch } to it.
                let mut drag_winner = None;
                for &id in &hit_path {
                    let accepts = tree
                        .borrow()
                        .get_element(id)
                        .and_then(|n| n.element.as_ref())
                        .map(|e| e.has_on_gesture() && e.accepts_device(PointerDeviceKind::Touch))
                        .unwrap_or(false);
                    if !accepts {
                        continue;
                    }
                    let local = local_position(tree, id, down_position);
                    dispatch_gesture_event(
                        cx,
                        id,
                        &ComposedGestureEvent::PointerDown {
                            local,
                            global: down_position,
                            button: MouseButton::Left,
                            device: PointerDeviceKind::Touch,
                        },
                    );
                    drag_winner = Some(id);
                    break;
                }

                if let Some(target) = drag_winner {
                    // Drag won — set up capture and dispatch PointerMove.
                    tracing::info!("SLOP: drag won target={target:?}");
                    self.arena.resolve(ArenaWinnerKind::Drag);
                    self.composer
                        .on_pointer_down(Some(target), hit_path.clone());
                    for &id in &hit_path {
                        let local = local_position(tree, id, position);
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
                        root,
                        delta_x: -dx,
                        delta_y: -dy,
                        position,
                    });
                }
            }
            TouchMoveOutcome::DragMoved => {
                let path: Vec<ElementNodeId> = self.composer.pointer_down_path().to_vec();
                for id in &path {
                    let local = local_position(tree, *id, position);
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
                    root,
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
        tree: &NodeTree,
        root: ViewRootId,
        position: Offset,
        time_ms: u64,
    ) {
        let outcome = self.arena.on_touch_up(position, time_ms);
        tracing::info!(
            "TOUCH UP at ({},{}) t={time_ms} outcome={:?}",
            position.x,
            position.y,
            outcome_kind(&outcome)
        );
        match outcome {
            TouchUpOutcome::DragEnded => {
                let path: Vec<ElementNodeId> = self.composer.pointer_down_path().to_vec();
                for id in &path {
                    let local = local_position(tree, *id, position);
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
                push_fling(cx.app_event_queue, root, -vx, -vy, position);
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
                self.handle_mouse_pointer_down(
                    cx,
                    tree,
                    position,
                    MouseButton::Left,
                    time_ms,
                    PointerDeviceKind::Touch,
                );
                self.handle_mouse_pointer_up(
                    cx,
                    tree,
                    position,
                    MouseButton::Left,
                    PointerDeviceKind::Touch,
                );
            }
            TouchUpOutcome::Idle => {}
        }
    }

    fn handle_touch_pointer_cancel(&mut self, cx: &mut SubsystemFlushContext<'_>, tree: &NodeTree) {
        let outcome = self.arena.on_touch_cancel();
        match outcome {
            TouchCancelOutcome::DragCanceled => {
                let path: Vec<ElementNodeId> = self.composer.pointer_down_path().to_vec();
                for id in &path {
                    let local = local_position(tree, *id, Offset::new(0.0, 0.0));
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
fn dispatch_click(
    cx: &mut SubsystemFlushContext<'_>,
    tree: &NodeTree,
    position: Offset,
    device: PointerDeviceKind,
) {
    let hit_path = HitTest::new(&tree.borrow()).path(position);
    for node_id in &hit_path {
        let local = local_position(tree, *node_id, position);
        dispatch_gesture_event(
            cx,
            *node_id,
            &ComposedGestureEvent::Click {
                local,
                global: position,
                device,
            },
        );
        if is_click_opaque(&tree.borrow(), *node_id) {
            break;
        }
    }
}

/// Dispatch `ComposedGestureEvent::ContextMenu` to every element in the
/// hit-path (mirrors how the web `contextmenu` event bubbles).
fn dispatch_context_menu(
    cx: &mut SubsystemFlushContext<'_>,
    tree: &NodeTree,
    position: Offset,
    device: PointerDeviceKind,
) {
    let hit_path = HitTest::new(&tree.borrow()).path(position);
    for node_id in &hit_path {
        let local = local_position(tree, *node_id, position);
        dispatch_gesture_event(
            cx,
            *node_id,
            &ComposedGestureEvent::ContextMenu {
                local,
                global: position,
                device,
            },
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

fn local_position(tree: &NodeTree, node_id: ElementNodeId, global: Offset) -> Offset {
    let mut abs_x = 0.0f64;
    let mut abs_y = 0.0f64;
    let tree = tree.borrow();
    let mut current: Option<NodeId> = Some(node_id.into());
    while let Some(cid) = current {
        if let Some(n) = tree.get_element(cid.as_element_id()) {
            abs_x += n.computed_layout.offset.x;
            abs_y += n.computed_layout.offset.y;
            current = n.parent;
        } else if let Some(f) = tree.get_fragment(cid.as_fragment_id()) {
            current = Some(f.parent);
        } else {
            break;
        }
    }
    Offset::new(global.x - abs_x, global.y - abs_y)
}

/// Dispatch a gesture event to an element and mark it dirty for re-layout
/// (the handler may have pushed mutations / requested focus or paint).
fn dispatch_gesture_event(
    cx: &mut SubsystemFlushContext<'_>,
    id: ElementNodeId,
    event: &ComposedGestureEvent,
) {
    let Some(tree) = cx.tree_containing(id.into()) else {
        return;
    };
    let mut t = tree.borrow_mut();
    let Some(node) = t.get_element_mut(id) else {
        return;
    };
    let Some(ref mut element) = node.element else {
        return;
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
    element.on_gesture_event(&mut el_cx, event);
    t.mark_dirty(id.into());
}
