use crate::core::elements::{ComposedGestureEvent, ElementOnGestureContext};
use crate::core::event::{AppEvent, AppGestureEvent};
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::hit_test::HitTest;
use crate::elements::pointer_interact::PointerInteractEvent;
use crate::elements::PointerInteractElement;
use tur_shared::{MouseButton, Offset};

pub struct GestureAppHandler;

impl AppHandler for GestureAppHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        match event {
            AppEvent::Gesture(AppGestureEvent::PointerDown { position, button, time_ms }) => {
                handle_pointer_down(cx, *position, *button, *time_ms);
            }
            AppEvent::Gesture(AppGestureEvent::PointerMove { position }) => {
                handle_pointer_move(cx, *position);
            }
            AppEvent::Gesture(AppGestureEvent::PointerUp { position, button }) => {
                handle_pointer_up(cx, *position, *button);
            }
            AppEvent::Gesture(AppGestureEvent::ContextMenu { position }) => {
                handle_context_menu(cx, *position);
            }
            _ => {}
        }
    }
}

fn handle_context_menu(cx: &mut HandlerContext, position: Offset) {
    let path = HitTest::new(&*cx.element_tree).path(position);
    // Dispatch ContextMenu to every element in the hit-path. The deepest
    // element gets first crack; outer elements receive it too so a wrapping
    // widget can show the menu on behalf of an inner one. Each element's
    // `onContextMenu` mutation (if any) is invoked with the local + global
    // positions.
    for id in &path {
        let local = local_position(cx, *id, position);
        dispatch_gesture_event(
            cx,
            *id,
            &ComposedGestureEvent::ContextMenu { local, global: position },
        );
    }
}

fn handle_pointer_down(cx: &mut HandlerContext, position: Offset, button: MouseButton, time_ms: u64) {
    let path = HitTest::new(&*cx.element_tree).path(position);
    // Path is ordered [deepest, ..., outermost]; the deepest hit is the
    // primary gesture target.
    let target = path.first().copied();
    cx.gesture_composer.on_pointer_down(target, path.clone());

    // Engine-side multi-click classification: compare this click against
    // recent history to decide whether to dispatch PointerDown (single),
    // PointerDoubleDown, or PointerTripleDown.
    let kind = cx.gesture_composer.classify_click(position, time_ms);
    for id in &path {
        let local = local_position(cx, *id, position);
        let event = match kind {
            crate::core::gesture::ClickKind::Single => {
                ComposedGestureEvent::PointerDown { local, global: position, button }
            }
            crate::core::gesture::ClickKind::Double => {
                ComposedGestureEvent::PointerDoubleDown { local, global: position, button }
            }
            crate::core::gesture::ClickKind::Triple => {
                ComposedGestureEvent::PointerTripleDown { local, global: position, button }
            }
        };
        dispatch_gesture_event(cx, *id, &event);
    }
}

fn handle_pointer_move(cx: &mut HandlerContext, position: Offset) {
    if !cx.gesture_composer.is_tracking_drag() {
        return;
    }

    // Route move events to the elements that received the original pointer-down
    // (gesture capture): even if the pointer has moved off them, they continue
    // to receive moves until the drag ends.
    let path: Vec<ElementNodeId> = cx.gesture_composer.pointer_down_path().to_vec();
    for id in &path {
        let local = local_position(cx, *id, position);
        dispatch_gesture_event(
            cx,
            *id,
            &ComposedGestureEvent::PointerMove { local, global: position },
        );
    }
}

fn handle_pointer_up(cx: &mut HandlerContext, position: Offset, button: MouseButton) {
    let down_target = cx.gesture_composer.pointer_down_target();
    let down_path: Vec<ElementNodeId> = cx.gesture_composer.pointer_down_path().to_vec();

    // Dispatch PointerUp to every element that received the pointer-down
    // (gesture capture) before click resolution.
    for id in &down_path {
        let local = local_position(cx, *id, position);
        dispatch_gesture_event(
            cx,
            *id,
            &ComposedGestureEvent::PointerUp { local, global: position, button },
        );
    }

    let click_eligible = match down_target {
        Some(id) => HitTest::new(&*cx.element_tree).contains(position, id),
        None => false,
    };

    let clicked = cx.gesture_composer.on_pointer_up(click_eligible).is_some();
    if clicked {
        let hit_path = HitTest::new(&*cx.element_tree).path(position);
        for node_id in &hit_path {
            if let Some(node) = cx.element_tree.get_element(*node_id) {
                if let Some(ref element) = node.element {
                    if let Some(p) = element.cast::<PointerInteractElement>() {
                        if let Some(m) = p.component.on_click {
                            let local = local_position(cx, *node_id, position);
                            cx.mutation_queue.push(
                                m,
                                PointerInteractEvent { local, global: position },
                            );
                        }
                    }
                }
            }
            if is_click_opaque(&*cx.element_tree, *node_id) {
                break;
            }
        }
    }
}

fn is_click_opaque(tree: &crate::core::elements::ElementTree, id: ElementNodeId) -> bool {
    tree.get_element(id)
        .and_then(|node| node.element.as_ref())
        .map(|e| {
            e.cast::<PointerInteractElement>()
                .map(|p| p.is_click_opaque())
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Compute a position relative to the element's top-left by walking parents
/// and subtracting each one's layout offset. Walks through fragment ancestors
/// transparently (fragments have zero offset, so they're skipped without
/// affecting the sum).
fn local_position(cx: &HandlerContext, node_id: ElementNodeId, global: Offset) -> Offset {
    let mut abs_x = 0.0f64;
    let mut abs_y = 0.0f64;
    let mut current: Option<NodeId> = Some(node_id.into());
    while let Some(cid) = current {
        if let Some(n) = cx.element_tree.get_element(ElementNodeId::new(cid.as_u64())) {
            abs_x += n.computed_layout.offset.x;
            abs_y += n.computed_layout.offset.y;
            current = n.parent;
        } else if let Some(f) = cx.element_tree.get_fragment(crate::core::element::FragmentNodeId::new(cid.as_u64())) {
            // Fragments have zero offset; hop to their real-ancestor parent.
            current = Some(f.parent);
        } else {
            break;
        }
    }
    Offset::new(global.x - abs_x, global.y - abs_y)
}

fn dispatch_gesture_event(cx: &mut HandlerContext, id: ElementNodeId, event: &ComposedGestureEvent) {
    let Some(node) = cx.element_tree.get_element_mut(id) else {
        return;
    };
    let Some(ref mut element) = node.element else {
        return;
    };
    let mut el_cx = ElementOnGestureContext::new(
        &mut *cx.event_queue,
        &mut *cx.focus_manager,
        &mut *cx.mutation_queue,
        id,
    );
    element.on_gesture_event(&mut el_cx, event);
    cx.element_tree.mark_dirty(id.into());
}
