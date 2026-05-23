use crate::core::elements::ElementOnKeyboardContext;
use crate::core::event::AppEvent;
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::keyboard::make_key_down_command;

pub struct KeyboardAppHandler;

impl AppHandler for KeyboardAppHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::Key(key_event) = event else {
            return;
        };

        dispatch_key_event(cx, key_event);

        let Some(focused_id) = cx.focus_manager.focused() else {
            return;
        };
        let mut current = Some(focused_id);
        while let Some(id) = current {
            cx.js_command_queue.push(id, make_key_down_command(key_event));
            current = cx.element_tree.parent_of(id);
        }
    }
}

fn dispatch_key_event(cx: &mut HandlerContext, event: &crate::core::keyboard::AppKeyEvent) {
    let Some(focused_id) = cx.focus_manager.focused() else {
        return;
    };
    let Some(node) = cx.element_tree.get_mut(focused_id) else {
        return;
    };
    let Some(ref mut element) = node.element else {
        return;
    };
    let mut el_cx = ElementOnKeyboardContext::new(
        &mut *cx.js_command_queue,
        &mut *cx.event_queue,
        focused_id,
    );
    element.on_keyboard_event(&mut el_cx, event);
    cx.element_tree.mark_dirty(focused_id);
}
