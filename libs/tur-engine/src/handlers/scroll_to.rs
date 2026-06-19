use crate::core::event::AppEvent;
use crate::core::handler::{AppHandler, HandlerContext};
use crate::elements::ScrollViewElement;
use crate::handlers::wheel::dispatch_wheel;

/// Resolves `AppEvent::ScrollTo` by translating the requested absolute offset
/// into a delta and routing it through the wheel path (`dispatch_wheel`), which
/// clamps, updates controller metrics, fires `onScroll`, requests a redraw,
/// and handles scroll chaining.
pub struct ScrollToHandler;

impl AppHandler for ScrollToHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::ScrollTo { node_id, offset } = event else {
            return;
        };
        let (node_id, offset) = (*node_id, *offset);

        let current = cx
            .element_tree
            .get(node_id)
            .and_then(|n| n.element.as_ref())
            .and_then(|e| e.cast::<ScrollViewElement>())
            .map(|sv| sv.scroll_offset())
            .unwrap_or(0.0);

        let delta = offset - current;
        if delta.abs() > 0.001 {
            dispatch_wheel(cx, node_id, 0.0, delta);
        }
    }
}
