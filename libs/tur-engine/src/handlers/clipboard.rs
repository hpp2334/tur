use crate::core::element::ElementNodeId;
use crate::core::event::AppEvent;
use crate::core::handler::{AppHandler, HandlerContext};
use crate::elements::EditableTextElement;

/// Handles `AppEvent::ClipboardPaste` by inserting the pasted text into the
/// currently-focused editable text element (if any). The embedder (tur-wasm)
/// captures the user's Cmd+V paste event on a hidden input and forwards the
/// clipboard text via this event.
pub struct ClipboardPasteHandler;

impl AppHandler for ClipboardPasteHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::ClipboardPaste { text } = event else {
            return;
        };
        let text = text.clone();

        let Some(focused_id) = cx.focus_manager.focused() else {
            return;
        };

        // Take the element out of the tree so we can mutate it without
        // holding the tree borrow.
        let mut element_opt = {
            let Some(node) = cx.element_tree.get_element_mut(ElementNodeId::new(focused_id.as_u64())) else {
                return;
            };
            node.element.take()
        };
        let Some(mut element) = element_opt.take() else {
            return;
        };

        let mut did_change = false;
        if let Some(editable) = element.cast_mut::<EditableTextElement>() {
            // Replace any existing selection, otherwise insert at the cursor.
            let insert_at = if editable.controller().has_selection() {
                let (start, _end) = editable.controller().selection_range();
                editable.controller_mut().delete_range(start, _end);
                start
            } else {
                editable.controller().cursor_position()
            };
            {
                let mut c = editable.controller_mut();
                c.insert_str_at(insert_at, &text);
                let new_cursor = insert_at + text.len();
                c.set_cursor_position(new_cursor);
                c.set_selection(new_cursor, new_cursor);
            }
            did_change = true;
        }

        // Put the element back.
        if let Some(node) = cx.element_tree.get_element_mut(ElementNodeId::new(focused_id.as_u64())) {
            node.element = Some(element);
        }
        if did_change {
            cx.element_tree.mark_dirty(focused_id);
        }
    }
}

/// Handles `AppEvent::ClipboardWrite` (produced by EditableText on Cmd+C /
/// Cmd+X) by stashing the text into a poll slot. The embedder drains the
/// slot via `TurApp::take_clipboard_write()` once per frame and performs the
/// real system-clipboard write (e.g. `navigator.clipboard.writeText` in
/// tur-wasm). This split mirrors the existing `current_cursor` pattern and
/// keeps the engine free of any direct embedder dependency.
pub struct ClipboardWriteHandler;

impl AppHandler for ClipboardWriteHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::ClipboardWrite { text } = event else {
            return;
        };
        cx.push_clipboard_write(text.clone());
    }
}
