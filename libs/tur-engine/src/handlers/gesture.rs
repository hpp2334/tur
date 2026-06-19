use crate::core::elements::{ComposedGestureEvent, ElementOnGestureContext};
use crate::core::event::{AppEvent, AppGestureEvent};
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::hit_test::HitTest;
use crate::core::element::ElementNodeId;
use crate::elements::pointer_interact::ClickEvent;
use crate::elements::PointerInteractElement;
use tur_shared::Offset;

pub struct GestureAppHandler;

impl AppHandler for GestureAppHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        match event {
            AppEvent::Gesture(AppGestureEvent::PointerDown { position }) => {
                handle_pointer_down(cx, *position);
            }
            AppEvent::Gesture(AppGestureEvent::PointerMove { position }) => {
                handle_pointer_move(cx, *position);
            }
            AppEvent::Gesture(AppGestureEvent::PointerUp { position }) => {
                handle_pointer_up(cx, *position);
            }
            _ => {}
        }
    }
}

fn handle_pointer_down(cx: &mut HandlerContext, position: Offset) {
    let target = HitTest::new(&*cx.element_tree).deepest(position);
    cx.gesture_composer.on_pointer_down(target);

    if let Some(id) = target {
        let local = local_position(cx, id, position);
        dispatch_gesture_event(cx, id, &ComposedGestureEvent::PointerDown { local_position: local });
    }
}

fn handle_pointer_move(cx: &mut HandlerContext, position: Offset) {
    let is_dragging = cx.gesture_composer.is_tracking_drag();
    if !is_dragging {
        return;
    }

    let Some(id) = cx.focus_manager.focused() else {
        return;
    };
    let local = local_position(cx, id, position);
    dispatch_gesture_event(cx, id, &ComposedGestureEvent::PointerMove { local_position: local });
}

fn handle_pointer_up(cx: &mut HandlerContext, position: Offset) {
    let down_target = cx.gesture_composer.pointer_down_target();
    let click_eligible = match down_target {
        Some(id) => HitTest::new(&*cx.element_tree).contains(position, id),
        None => false,
    };

    let clicked = cx.gesture_composer.on_pointer_up(click_eligible).is_some();
    if clicked {
        let hit_path = HitTest::new(&*cx.element_tree).path(position);
        for node_id in &hit_path {
            if let Some(node) = cx.element_tree.get(*node_id) {
                if let Some(ref element) = node.element {
                    if let Some(p) = element.cast::<PointerInteractElement>() {
                        if let Some(m) = p.component.on_click {
                            cx.mutation_queue
                                .push(m, ClickEvent { x: position.x, y: position.y });
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
    tree.get(id)
        .and_then(|node| node.element.as_ref())
        .map(|e| {
            e.cast::<PointerInteractElement>()
                .map(|p| p.is_click_opaque())
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn local_position(cx: &HandlerContext, node_id: ElementNodeId, global: Offset) -> Offset {
    let mut abs_x = 0.0f64;
    let mut abs_y = 0.0f64;
    let mut current = Some(node_id);
    while let Some(cid) = current {
        if let Some(n) = cx.element_tree.get(cid) {
            abs_x += n.computed_layout.offset.x;
            abs_y += n.computed_layout.offset.y;
            current = n.parent;
        } else {
            break;
        }
    }
    Offset::new(global.x - abs_x, global.y - abs_y)
}

fn dispatch_gesture_event(cx: &mut HandlerContext, id: ElementNodeId, event: &ComposedGestureEvent) {
    let Some(node) = cx.element_tree.get_mut(id) else {
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
    cx.element_tree.mark_dirty(id);
}
