use crate::core::element::ElementNodeId;
use crate::core::elements::{ElementOnWheelContext, WheelEvent};
use crate::core::event::{AppEvent, PlatformEvent};
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::hit_test::HitTest;
use crate::core::layout::Offset;

pub struct WheelAppHandler;

impl AppHandler for WheelAppHandler {
    fn handle_platform_event(&mut self, cx: &mut HandlerContext, event: &PlatformEvent) {
        // Real device wheel / trackpad scroll from the platform.
        let PlatformEvent::Wheel { delta_x, delta_y, position } = event else {
            return;
        };
        process_scroll_delta(cx, *delta_x, *delta_y, *position);
    }

    fn handle_app_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        // Derived scroll produced by the gesture arena (e.g. a touch drag the
        // arena resolved to scroll). Routed through the same pipeline as a
        // real platform wheel so hit-testing, overscroll and chaining behave
        // identically.
        let AppEvent::Scroll { delta_x, delta_y, position } = event else {
            return;
        };
        process_scroll_delta(cx, *delta_x, *delta_y, *position);
    }
}

/// Shared scroll-delta processing for real (`PlatformEvent::Wheel`) and
/// derived (`AppEvent::Scroll`) scroll: hit-test to the deepest wheel-bearing
/// element, dispatch the delta, and forward any residual as overscroll.
fn process_scroll_delta(cx: &mut HandlerContext, delta_x: f64, delta_y: f64, position: Offset) {
    let hit_path = HitTest::new(&*cx.element_tree).path(position);
    let target = find_deepest_with_wheel(&*cx.element_tree, &hit_path);

    let Some(target_id) = target else {
        return;
    };

    let overscroll = dispatch_wheel(cx, target_id, delta_x, delta_y);
    if overscroll.abs() > 0.001 {
        cx.app_event_queue.push(AppEvent::ScrollOverscroll {
            source_id: target_id,
            delta: overscroll,
        });
    }
}

fn find_deepest_with_wheel(
    tree: &crate::core::elements::NodeTreeData,
    hit_path: &[ElementNodeId],
) -> Option<ElementNodeId> {
    for &id in hit_path {
        if let Some(node) = tree.get_element(id)
            && let Some(ref element) = node.element
                && element.has_on_wheel() {
                    return Some(id);
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
    let Some(node) = cx.element_tree.get_element_mut(id) else {
        return 0.0;
    };
    let Some(ref mut element) = node.element else {
        return 0.0;
    };
    let mut el_cx = ElementOnWheelContext::new(
        &mut *cx.app_event_queue,
        &mut *cx.mutation_queue,
        cx.need_paint,
        id,
    );
    let overscroll = element.on_wheel_event(&mut el_cx, &WheelEvent { delta_x, delta_y });
    cx.element_tree.mark_dirty(id.into());
    overscroll
}
