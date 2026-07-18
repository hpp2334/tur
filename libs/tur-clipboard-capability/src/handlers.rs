//! Engine-internal clipboard event handlers — the Cmd+C/Cmd+V/Cmd+X path.
//!
//! Moved here from the engine's clipboard handler module so that clipboard
//! concerns (bridge + handlers + trait) live in a single crate. The engine's
//! `TurStdPlugin` no longer registers these handlers itself —
//! [`crate::TurClipboardPlugin`] does, after declaring `requires::<Clipboard>`.

use tur_engine::core::event::{AppEvent, PlatformEvent};
use tur_engine::core::handler::{AppHandler, HandlerContext};
use tur_engine::elements::editable_text::EditableTextElement;

use crate::Clipboard;

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

    fn handle_app_event(&mut self, _cx: &mut HandlerContext, _event: &AppEvent) {}
}

/// Handles `AppEvent::ClipboardWrite` (produced by EditableText on Cmd+C /
/// Cmd+X) by spawning `clipboard.write_text(text)` on the engine's async
/// executor. Looks up the [`Clipboard`] capability at dispatch time via
/// `cx.capabilities.of::<Clipboard>()`; silently drops the write (with a
/// warning) if no backend is registered — though `TurClipboardPlugin`'s
/// `requires` declaration should prevent that at `build()` time.
///
/// The async executor is sourced from [`HandlerContext`] at dispatch time, so
/// this handler holds no executor state of its own.
pub struct ClipboardWriteHandler;

impl AppHandler for ClipboardWriteHandler {
    fn handle_app_event(&mut self, cx: &mut HandlerContext, event: &AppEvent) {
        let AppEvent::ClipboardWrite { text } = event else {
            return;
        };
        let text = text.clone();
        let Some(clipboard_cap) = cx.capabilities.of::<Clipboard>() else {
            tracing::warn!(
                "ClipboardWrite dropped: no Clipboard capability registered"
            );
            return;
        };
        let backend = clipboard_cap.backend().clone();
        cx.async_executor.spawn_detached(async move {
            backend.write_text(text).await;
        });
    }

    fn handle_platform_event(&mut self, _cx: &mut HandlerContext, _event: &PlatformEvent) {}
}
