use crate::core::app::AppEvent;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};
use crate::builtin_plugins::clipboard::ClipboardPasteEvent;

use crate::builtin_plugins::text::controller::{CursorChangeEvent, InputEvent};
use crate::builtin_plugins::text::elements::editable_text::EditableTextElement;

/// Consumes a [`ClipboardPasteEvent`] (forwarded by tur-clipboard's
/// [`ClipboardPlatformSubsystem`](crate::builtin_plugins::clipboard::handlers::ClipboardPlatformSubsystem)
/// from the embedder's [`ClipboardPlatformPasteEvent`]) and inserts the
/// pasted text into the focused [`EditableTextElement`], replacing any
/// active selection or inserting at the caret.
///
/// Lives in tur-text (next to `EditableTextElement`) rather than as a
/// per-element trait in the engine: paste is a stateless, single-consumer
/// operation, so a dedicated custom AppEvent + subsystem is simpler than a
/// vtable slot on every `AnyElement`.
///
/// Caret visibility after a paste is handled separately by
/// [`CaretVisibilitySubsystem`](super::CaretVisibilitySubsystem), which
/// subscribes to the same [`ClipboardPasteEvent`] and must be registered
/// after this subsystem so the caret move it observes is the post-paste one.
///
/// [`ClipboardPlatformPasteEvent`]: crate::builtin_plugins::clipboard::ClipboardPlatformPasteEvent
pub struct ClipboardPasteSubsystem;

impl Subsystem for ClipboardPasteSubsystem {
    fn handle_app_event(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        event: &AppEvent,
    ) {
        let Some(ev) = event.as_custom::<ClipboardPasteEvent>() else {
            return;
        };
        let text = ev.text.clone();

        let Some(focused_id) = cx.focus_manager.borrow().focused() else {
            return;
        };

        // Take the element out of the tree so we can mutate it without
        // holding the tree borrow while we access the controller.
        let mut element_opt = {
            let mut tree = cx.element_tree.borrow_mut();
            let Some(node) = tree.get_element_mut(focused_id) else {
                return;
            };
            node.element.take()
        };

        let mut did_change = false;
        if let Some(editable) = element_opt
            .as_mut()
            .and_then(|e| e.cast_mut::<EditableTextElement>())
        {
            // Replace any active selection, otherwise insert at the caret.
            // Records undo history via the controller's mutating methods
            // (suppressed while the undo controller itself applies a
            // restored value).
            let insert_at = if editable.controller().has_selection() {
                let (start, _end) = editable.controller().selection_range();
                editable.controller_mut().delete_range(start, _end);
                start
            } else {
                editable.controller().cursor_position()
            };

            let prev_text = editable.controller().text();

            {
                let mut c = editable.controller_mut();
                c.insert_str_at(insert_at, &text);
                let new_cursor = insert_at + text.len();
                c.set_cursor_position(new_cursor);
                c.set_selection(new_cursor, new_cursor);
            }

            let new_text = editable.controller().text();
            if new_text != prev_text
                && let Some(m) = editable.controller().on_input()
            {
                let mut mq = cx.mutation_queue.borrow_mut();
                mq.push(m, InputEvent { value: new_text, enter: false });
            }
            let cursor = editable.controller().cursor_position();
            if let Some(m) = editable.controller().on_cursor_change() {
                let mut mq = cx.mutation_queue.borrow_mut();
                mq.push(m, CursorChangeEvent { position: cursor });
            }
            cx.request_paint();
            did_change = true;
        }

        // Put the element back.
        let mut tree = cx.element_tree.borrow_mut();
        if let Some(node) = tree.get_element_mut(focused_id) {
            node.element = element_opt;
        }
        if did_change {
            tree.mark_dirty(focused_id.into());
        }
    }
}
