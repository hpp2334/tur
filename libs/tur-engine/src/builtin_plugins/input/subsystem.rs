use crate::core::element::NodeId;
use crate::core::elements::ElementOnKeyboardContext;
use crate::core::platform::key_event::KeyEvent;
use crate::core::platform::key_event::KeydownEvent;
use crate::core::platform::{PlatformEvent, ShellEventPayload};
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

use crate::builtin_plugins::focus::focusable::FocusableElement;

/// Routes key shell events to the focused element synchronously (mutating
/// the element's text buffer / caret in place), then bubbles the key event up
/// the focus chain via `onKeyDown` mutations.
///
/// Caret-keep-visible behaviour (scrolling the nearest scrollable ancestor
/// to keep the caret on screen after a key event) lives in tur-text's
/// `CaretVisibilitySubsystem`, which must be registered after this subsystem
/// so it observes the post-event caret position.
pub struct KeyboardSubsystem;

impl Subsystem for KeyboardSubsystem {
    fn handle_platform_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &PlatformEvent) {
        let ShellEventPayload::Key(key_event) = event.payload() else {
            return;
        };

        dispatch_key_event(cx, key_event);

        bubble_on_key_down(cx, key_event);
    }
}

fn dispatch_key_event(cx: &mut SubsystemFlushContext<'_>, event: &KeyEvent) {
    let Some(focused_id) = cx.focus_manager.borrow().focused() else {
        return;
    };
    let Some(tree_handle) = cx.tree_containing(focused_id.into()) else {
        return;
    };
    let mut tree = tree_handle.borrow_mut();
    let Some(node) = tree.get_element_mut(focused_id) else {
        return;
    };
    let Some(ref mut element) = node.element else {
        return;
    };
    let mut mq = cx.mutation_queue.borrow_mut();
    let mut el_cx = ElementOnKeyboardContext::new(&mut mq, &mut *cx.app_event_queue, cx.need_paint);
    element.on_keyboard_event(&mut el_cx, event);
    tree.mark_dirty(focused_id.into());
}

fn bubble_on_key_down(cx: &mut SubsystemFlushContext<'_>, key_event: &KeyEvent) {
    let Some(focused_id) = cx.focus_manager.borrow().focused() else {
        return;
    };
    let key = key_event.key.clone();
    let code = key_event.code.clone();
    let modifiers = key_event.modifiers;
    // Walk from the focused element up to the root, hopping fragment links
    // transparently (fragments can't host Focusable handlers themselves).
    let mut mq = cx.mutation_queue.borrow_mut();
    let Some(tree_handle) = cx.tree_containing(focused_id.into()) else {
        return;
    };
    let tree = tree_handle.borrow();
    let mut current: Option<NodeId> = Some(focused_id.into());
    while let Some(nid) = current {
        if let Some(node) = tree.get_element(nid.as_element_id()) {
            if let Some(ref element) = node.element
                && let Some(f) = element.cast::<FocusableElement>()
                && let Some(m) = f.view.on_key_down
            {
                mq.push(
                    m,
                    KeydownEvent {
                        key: key.clone(),
                        code: code.clone(),
                        modifiers,
                    },
                );
            }
            current = node.parent;
        } else if let Some(frag) = tree.get_fragment(nid.as_fragment_id()) {
            current = Some(frag.parent);
        } else {
            break;
        }
    }
}
