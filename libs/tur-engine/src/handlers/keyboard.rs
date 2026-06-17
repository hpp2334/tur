use crate::core::elements::ElementOnKeyboardContext;
use crate::core::event::AppEvent;
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::keyboard::KeydownEvent;
use crate::elements::Focusable;

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
        let key = key_event.key.clone();
        let code = key_event.code.clone();
        let modifiers = key_event.modifiers;
        let mut current = Some(focused_id);
        while let Some(id) = current {
            if let Some(node) = cx.element_tree.get(id) {
                if let Some(ref element) = node.element {
                    if let Some(f) = element.cast::<Focusable>() {
                        if let Some(m) = f.spec.on_key_down {
                            cx.mutation_queue.push(
                                m,
                                KeydownEvent {
                                    key: key.clone(),
                                    code: code.clone(),
                                    modifiers,
                                },
                            );
                        }
                    }
                }
            }
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
        &mut *cx.mutation_queue,
        &mut *cx.event_queue,
    );
    element.on_keyboard_event(&mut el_cx, event);
    cx.element_tree.mark_dirty(focused_id);
}
