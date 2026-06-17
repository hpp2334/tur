use crate::core::element::ElementNodeId;
use crate::core::elements::{ElementOnWheelContext, WheelEvent};
use crate::core::event::AppEvent;
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::hit_test::HitTest;

pub struct WheelAppHandler;

impl AppHandler for WheelAppHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::Wheel { delta_x, delta_y, position } = event else {
            return;
        };
        let (delta_x, delta_y, position) = (*delta_x, *delta_y, *position);

        let hit_path = HitTest::new(&*cx.element_tree).path(position);
        let target = find_deepest_with_wheel(&*cx.element_tree, &hit_path);

        let Some(target_id) = target else {
            return;
        };

        let overscroll = dispatch_wheel(cx, target_id, delta_x, delta_y);
        if overscroll.abs() > 0.001 {
            cx.event_queue.push(AppEvent::ScrollOverscroll {
                source_id: target_id,
                delta: overscroll,
            });
        }
    }
}

fn find_deepest_with_wheel(
    tree: &crate::core::elements::ElementTree,
    hit_path: &[ElementNodeId],
) -> Option<ElementNodeId> {
    for &id in hit_path {
        if let Some(node) = tree.get(id) {
            if let Some(ref element) = node.element {
                if element.has_on_wheel() {
                    return Some(id);
                }
            }
        }
    }
    None
}

pub fn dispatch_wheel(
    cx: &mut HandlerContext,
    id: ElementNodeId,
    delta_x: f64,
    delta_y: f64,
) -> f64 {
    let Some(node) = cx.element_tree.get_mut(id) else {
        return 0.0;
    };
    let Some(ref mut element) = node.element else {
        return 0.0;
    };
    let mut el_cx = ElementOnWheelContext::new(
        &mut *cx.event_queue,
        &mut *cx.mutation_queue,
        id,
    );
    let overscroll = element.on_wheel_event(&mut el_cx, &WheelEvent { delta_x, delta_y });
    cx.element_tree.mark_dirty(id);
    overscroll
}
