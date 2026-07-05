use tur_engine::core::elements::ElementOnImeContext;
use tur_engine::core::event::AppEvent;
use crate::handlers::ensure_visible::ensure_caret_visible;

pub struct ImeAppHandler;

impl tur_engine::core::handler::AppHandler for ImeAppHandler {
    fn handle_event(
        &mut self,
        cx: &mut tur_engine::core::handler::HandlerContext,
        event: &AppEvent,
    ) {
        let AppEvent::Ime(ime_event) = event else {
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
                ElementOnImeContext::new(&mut *cx.mutation_queue, &mut *cx.event_queue);
            element.on_ime_event(&mut el_cx, ime_event);
        }
        cx.element_tree.mark_dirty(focused_id.into());

        // A composition end inserts text and moves the caret; keep it visible.
        ensure_caret_visible(cx);
    }
}
