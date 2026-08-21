use crate::core::elements::ElementOnImeContext;
use crate::core::platform::PlatformEvent;
use crate::core::shell::ShellEvent;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

pub struct ImeSubsystem;

impl Subsystem for ImeSubsystem {
    fn handle_platform_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &PlatformEvent) {
        let PlatformEvent::Shell(ShellEvent::Ime(ime_event)) = event else {
            return;
        };

        let Some(focused_id) = cx.focus_manager.borrow().focused() else {
            return;
        };
        {
            let mut tree = cx.element_tree.borrow_mut();
            let Some(node) = tree.get_element_mut(focused_id) else {
                return;
            };
            let Some(ref mut element) = node.element else {
                return;
            };

            let mut mq = cx.mutation_queue.borrow_mut();
            let mut el_cx = ElementOnImeContext::new(&mut mq, cx.need_paint);
            element.on_ime_event(&mut el_cx, ime_event);
        }
        cx.element_tree.borrow_mut().mark_dirty(focused_id.into());

        // Keeping the caret visible after composition-end is handled by
        // tur-text's CaretVisibilitySubsystem, which runs after this
        // subsystem in registration order.
    }
}
