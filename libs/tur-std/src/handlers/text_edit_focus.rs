use tur_engine::core::event::{AppEvent, AppGestureEvent};
use tur_engine::core::focus::helper::find_focusable_in_path;
use tur_engine::core::handler::{AppHandler, HandlerContext};
use tur_engine::core::hit_test::HitTest;

pub struct TextEditFocusAppHandler;

impl AppHandler for TextEditFocusAppHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::Gesture(AppGestureEvent::PointerUp { position, .. }) = event else {
            return;
        };

        let hit_path = HitTest::new(&*cx.element_tree).path(*position);
        let focusable_id = find_focusable_in_path(&*cx.element_tree, &hit_path);

        if focusable_id.is_none() {
            if let Some(focused) = cx.focus_manager.focused() {
                if cx.gesture_composer.pointer_down_target() == Some(focused) {
                    return;
                }
            }
            cx.focus_manager.clear_focus();
        }
    }
}
