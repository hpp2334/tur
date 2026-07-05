use tur_engine::core::elements::{ComposedGestureEvent, ElementOnGestureContext};
use tur_engine::core::event::{AppEvent, AppGestureEvent};
use tur_engine::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use tur_engine::core::gesture::ClickKind;
use tur_engine::core::handler::{AppHandler, HandlerContext};
use tur_engine::core::hit_test::HitTest;
use tur_engine::elements::pointer_interact::{PointerInteractElement, PointerInteractEvent};
use tur_engine::core::elements::NodeTreeData;
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
    let target = path.first().copied();
    cx.gesture_composer.on_pointer_down(target, path.clone());

    let kind = cx.gesture_composer.classify_click(position, time_ms);
    for id in &path {
        let local = local_position(cx, *id, position);
        let event = match kind {
            ClickKind::Single => {
                ComposedGestureEvent::PointerDown { local, global: position, button }
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

fn handle_pointer_move(cx: &mut HandlerContext, position: Offset) {
    if !cx.gesture_composer.is_tracking_drag() {
        return;
    }

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
                        if let Some(m) = p.view.on_click {
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

fn local_position(cx: &HandlerContext, node_id: ElementNodeId, global: Offset) -> Offset {
    let mut abs_x = 0.0f64;
    let mut abs_y = 0.0f64;
    let mut current: Option<NodeId> = Some(node_id.into());
    while let Some(cid) = current {
        if let Some(n) = cx.element_tree.get_element(ElementNodeId::new(cid.as_u64())) {
            abs_x += n.computed_layout.offset.x;
            abs_y += n.computed_layout.offset.y;
            current = n.parent;
        } else if let Some(f) = cx.element_tree.get_fragment(FragmentNodeId::new(cid.as_u64())) {
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
