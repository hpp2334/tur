use tur_engine::core::element::{ElementNodeId, FragmentNodeId, NodeId};
use tur_engine::core::elements::ElementOnKeyboardContext;
use tur_engine::core::event::PlatformEvent;
use tur_engine::core::handler::{AppHandler, HandlerContext};
use tur_engine::core::keyboard::AppKeyEvent;
use crate::handlers::ensure_visible::ensure_caret_visible;

use crate::elements::focusable::FocusableElement;
use crate::keyboard::KeydownEvent;

pub struct KeyboardAppHandler;

impl AppHandler for KeyboardAppHandler {
    fn handle_platform_event(&mut self, cx: &mut HandlerContext, event: &PlatformEvent) {
        let PlatformEvent::Key(key_event) = event else {
            return;
        };

        dispatch_key_event(cx, key_event);

        // Keep the caret on screen after cursor-moving keys / typed text.
        ensure_caret_visible(cx);

        let Some(focused_id) = cx.focus_manager.focused() else {
            return;
        };
        let key = key_event.key.clone();
        let code = key_event.code.clone();
        let modifiers = key_event.modifiers;
        // Walk from the focused element up to the root, hopping fragment links
        // transparently (fragments can't host Focusable handlers themselves).
        let mut current: Option<NodeId> = Some(focused_id.into());
        while let Some(nid) = current {
            if let Some(node) = cx.element_tree.get_element(ElementNodeId::new(nid.as_u64())) {
                if let Some(ref element) = node.element {
                    if let Some(f) = element.cast::<FocusableElement>() {
                        if let Some(m) = f.view.on_key_down {
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
                current = node.parent;
            } else if let Some(frag) = cx
                .element_tree
                .get_fragment(FragmentNodeId::new(nid.as_u64()))
            {
                current = Some(frag.parent);
            } else {
                break;
            }
        }
    }
}

fn dispatch_key_event(cx: &mut HandlerContext, event: &AppKeyEvent) {
    let Some(focused_id) = cx.focus_manager.focused() else {
        return;
    };
    let Some(node) = cx.element_tree.get_element_mut(focused_id) else {
        return;
    };
    let Some(ref mut element) = node.element else {
        return;
    };
    let mut el_cx = ElementOnKeyboardContext::new(
        &mut *cx.mutation_queue,
        &mut *cx.app_event_queue,
    );
    element.on_keyboard_event(&mut el_cx, event);
    cx.element_tree.mark_dirty(focused_id.into());
}
