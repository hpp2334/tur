use crate::core::elements::ElementOnImeContext;
use crate::core::event::AppEvent;

pub struct ImeAppHandler;

impl crate::core::handler::AppHandler for ImeAppHandler {
    fn handle_event(
        &mut self,
        cx: &mut crate::core::handler::HandlerContext,
        event: &AppEvent,
    ) {
        let AppEvent::Ime(ime_event) = event else {
            return;
        };

        let Some(focused_id) = cx.focus_manager.focused() else {
            return;
        };
        let Some(node) = cx.element_tree.get_mut(focused_id) else {
            return;
        };
        let Some(ref mut element) = node.element else {
            return;
        };

        let mut el_cx = ElementOnImeContext::new(
            &mut *cx.js_command_queue,
            &mut *cx.event_queue,
            focused_id,
        );
        element.on_ime_event(&mut el_cx, ime_event);
    }
}
