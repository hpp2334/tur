use crate::core::event::{AppEvent, AppGestureEvent};
use crate::core::focus::helper::find_focusable_in_path;
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::hit_test::HitTest;

pub struct PointerFocusHandler;

impl AppHandler for PointerFocusHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::Gesture(AppGestureEvent::PointerUp { position }) = event else {
            return;
        };

        let hit_path = HitTest::new(cx.element_tree).path(*position);
        let focusable_id = find_focusable_in_path(cx.element_tree, &hit_path);
        if focusable_id.is_none() {
            cx.focus_manager.clear_focus(cx.js_event_queue);
        }
    }
}
