use crate::core::elements::ElementOnImeContext;
use crate::core::event::PlatformEvent;

pub struct ImeAppHandler;

impl crate::core::handler::AppHandler for ImeAppHandler {
    fn handle_platform_event(
        &mut self,
        cx: &mut crate::core::handler::HandlerContext,
        event: &PlatformEvent,
    ) {
        let PlatformEvent::Ime(ime_event) = event else {
            return;
        };

        let Some(focused_id) = cx.focus_manager.focused() else {
            return;
        };
        {
            let Some(node) = cx.element_tree.get_element_mut(focused_id) else {
                return;
            };
            let Some(ref mut element) = node.element else {
                return;
            };

            let mut el_cx =
                ElementOnImeContext::new(&mut *cx.mutation_queue, cx.need_paint);
            element.on_ime_event(&mut el_cx, ime_event);
        }
        cx.element_tree.mark_dirty(focused_id.into());

        // Keeping the caret visible after composition-end is handled by
        // tur-text's PostImeHandler, which runs after this handler in
        // registration order.
    }
}
