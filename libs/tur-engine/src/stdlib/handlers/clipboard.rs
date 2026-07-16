use std::rc::Rc;

use crate::core::async_::AsyncExecutor;
use crate::core::event::{AppEvent, PlatformEvent};
use crate::core::handler::{AppHandler, HandlerContext};
use crate::stdlib::elements::editable_text::EditableTextElement;
use crate::stdlib::platform::Clipboard;

/// Handles `PlatformEvent::ClipboardPaste` by inserting the pasted text into
/// the currently-focused editable text element (if any). The embedder
/// (tur-wasm) captures the user's Cmd+V paste event on a hidden input and
/// forwards the clipboard text via this event.
pub struct ClipboardPasteHandler;

impl AppHandler for ClipboardPasteHandler {
    fn handle_platform_event(&mut self, cx: &mut HandlerContext, event: &PlatformEvent) {
        let PlatformEvent::ClipboardPaste { text } = event else {
            return;
        };
        let text = text.clone();

        let Some(focused_id) = cx.focus_manager.focused() else {
            return;
        };

        // Take the element out of the tree so we can mutate it without
        // holding the tree borrow.
        let mut element_opt = {
            let Some(node) = cx.element_tree.get_element_mut(focused_id) else {
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
        if let Some(node) = cx.element_tree.get_element_mut(focused_id) {
            node.element = Some(element);
        }
        if did_change {
            cx.element_tree.mark_dirty(focused_id.into());
        }
    }
}

/// Handles `AppEvent::ClipboardWrite` (produced by EditableText on Cmd+C /
/// Cmd+X) by spawning `clipboard.write_text(text)` on the engine's async
/// executor. The future runs on the next `tick` pass; the write completes
/// asynchronously (browser `navigator.clipboard.writeText` on wasm, eager
/// on tests).
///
/// Unlike the previous slot-based design, this needs no embedder-side drain
/// loop — the executor drives the future inside `flush`.
pub struct ClipboardWriteHandler {
    clipboard: Rc<dyn Clipboard>,
    executor: Rc<AsyncExecutor>,
}

impl ClipboardWriteHandler {
    pub fn new(clipboard: Rc<dyn Clipboard>, executor: Rc<AsyncExecutor>) -> Self {
        Self { clipboard, executor }
    }
}

impl AppHandler for ClipboardWriteHandler {
    fn handle_app_event(&mut self, _cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::ClipboardWrite { text } = event else {
            return;
        };
        let text = text.clone();
        let clipboard = self.clipboard.clone();
        self.executor.spawn_detached(async move {
            clipboard.write_text(text).await;
        });
    }
}
