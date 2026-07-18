use crate::core::element::ElementNodeId;
use crate::core::elements::ElementOnClipboardContext;
use crate::core::event::PlatformEvent;
use crate::core::handler::{AppHandler, HandlerContext};

/// Dispatches `PlatformEvent::ClipboardPaste` to the currently-focused
/// element via its [`ElementOnClipboard`] impl (if any). Paste arrives via
/// a dedicated platform event — the embedder captures Cmd+V on a hidden
/// input and forwards the clipboard text — and this handler routes it to
/// the focused editable, which inserts the text at the caret (replacing
/// any selection).
///
/// Non-editable focused elements ignore the event (no trait impl wired
/// through `AnyElement::with_clipboard_paste`).
///
/// Keeping the caret on screen after a paste is handled separately by
/// tur-text's post-handler (registered after this handler in registration
/// order), which calls `ensure_caret_visible`.
pub struct ClipboardPasteAppHandler;

impl AppHandler for ClipboardPasteAppHandler {
    fn handle_platform_event(&mut self, cx: &mut HandlerContext, event: &PlatformEvent) {
        let PlatformEvent::ClipboardPaste { text } = event else {
            return;
        };
        let text = text.clone();

        let Some(focused_id) = cx.focus_manager.focused() else {
            return;
        };

        // Bail early if the focused element doesn't handle clipboard paste —
        // avoids the mutation_queue/need_paint borrows for non-editable
        // elements.
        {
            let Some(node) = cx.element_tree.get_element(focused_id) else {
                return;
            };
            let Some(ref element) = node.element else {
                return;
            };
            if !element.has_on_clipboard() {
                return;
            }
        }

        dispatch_clipboard_paste(cx, focused_id, &text);
    }
}

fn dispatch_clipboard_paste(cx: &mut HandlerContext, focused_id: ElementNodeId, text: &str) {
    let Some(node) = cx.element_tree.get_element_mut(focused_id) else {
        return;
    };
    let Some(ref mut element) = node.element else {
        return;
    };
    let mut el_cx = ElementOnClipboardContext::new(&mut *cx.mutation_queue, cx.need_paint);
    element.on_clipboard_paste(&mut el_cx, text);
    cx.element_tree.mark_dirty(focused_id.into());
}
