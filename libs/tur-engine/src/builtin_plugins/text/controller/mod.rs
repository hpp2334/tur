pub mod events;
pub mod span_data;
pub mod undo_controller;

pub use events::*;
pub use span_data::SpanData;
pub use undo_controller::{TextEditingValue, UndoController};

// ---------------------------------------------------------------------------
// TextEditingController — a plain Rust type shared as
// `Rc<RefCell<TextEditingController>>` between the element and whoever
// authors it (the rut rows in `core::rut_runtime`).
// ---------------------------------------------------------------------------

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::edgy::mutation::MutationHandle;
use crate::core::focus::{BlurEvent, FocusEvent};
use crate::core::platform::key_event::{KeydownEvent, KeyupEvent};

pub struct TextEditingController {
    spans: Vec<SpanData>,
    cursor_position: usize,
    selection_anchor: usize,
    selection_end: usize,
    composing_text: Option<String>,
    composing_start: usize,
    /// Content revision — bumped by every mutation that changes the *rendered
    /// text or span styles* (insert / delete / setSpans / clear / composition
    /// transitions). Pure caret/selection moves do NOT bump it, so the
    /// `EditableTextElement` layout memo (keyed on the revision) can skip
    /// re-shaping the whole document on cursor-only changes.
    revision: u64,
    /// Memoized join of `spans`, keyed by `revision`. `text()` is called
    /// several times per keystroke and once per painted frame; the join is
    /// O(document) so it is derived once per content change instead.
    cached_text: RefCell<Option<(u64, String)>>,
    /// Back-reference to the `UndoController` attached via `Input`'s
    /// `undoController` prop (bound at view-build time). When `Some`, every
    /// text-mutating method pushes a snapshot of the *current* state to the
    /// recorder BEFORE mutating — mirroring Flutter's `UndoHistory` listener
    /// model where recording is a side effect of the controller's value
    /// setter, not a per-call-site concern. Note: a controller shared across
    /// multiple `Input` elements will have its recorder overwritten by
    /// whichever element attaches last (the demo uses a single editor, so
    /// this is fine).
    undo_recorder: Option<Rc<RefCell<UndoController>>>,
    /// Transient flag set by the undo/redo keystroke arms while they apply a
    /// restored value via `set_spans_preserve_cursor`. Prevents the
    /// restoration from pushing the current state and clearing the redo
    /// stack. Single-threaded, no re-entrancy across the mutation boundary.
    suppress_undo: bool,
    on_input: Option<MutationHandle<InputEvent>>,
    on_cursor_change: Option<MutationHandle<CursorChangeEvent>>,
    on_selection_change: Option<MutationHandle<SelectionChangeEvent>>,
    on_key_down: Option<MutationHandle<KeydownEvent>>,
    on_key_up: Option<MutationHandle<KeyupEvent>>,
    on_focus: Option<MutationHandle<FocusEvent>>,
    on_blur: Option<MutationHandle<BlurEvent>>,
    on_composition_start: Option<MutationHandle<CompositionStartEvent>>,
    on_composition_update: Option<MutationHandle<CompositionUpdateEvent>>,
    on_composition_end: Option<MutationHandle<CompositionEndEvent>>,
}

impl TextEditingController {
    pub fn new() -> Self {
        Self {
            spans: Vec::new(),
            cursor_position: 0,
            selection_anchor: 0,
            selection_end: 0,
            composing_text: None,
            composing_start: 0,
            revision: 0,
            cached_text: RefCell::new(None),
            undo_recorder: None,
            suppress_undo: false,
            on_input: None,
            on_cursor_change: None,
            on_selection_change: None,
            on_key_down: None,
            on_key_up: None,
            on_focus: None,
            on_blur: None,
            on_composition_start: None,
            on_composition_update: None,
            on_composition_end: None,
        }
    }

    /// Current content revision. Consumers pair this with the rendered
    /// content they last derived (see `EditableTextElement`'s layout memo) so
    /// unchanged content skips re-shaping.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Bump the content revision (rendered text / span styles changed).
    /// Invalidates the memoized text join.
    fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        *self.cached_text.borrow_mut() = None;
    }

    /// Attach an `UndoController` recorder. Called by `EditableTextView`'s
    /// build when an undo controller is present, so the controller's
    /// text-mutating methods can snapshot to the history stack uniformly —
    /// regardless of whether the mutation originated from a keystroke, IME,
    /// paste, or a programmatic `set_spans`.
    pub fn set_undo_recorder(&mut self, recorder: Option<Rc<RefCell<UndoController>>>) {
        self.undo_recorder = recorder;
    }

    /// Temporarily suppress undo recording. Used by the Cmd+Z / Cmd+Shift+Z /
    /// Ctrl+Y arms while they apply a restored value: the restoration must
    /// not itself push a snapshot (it would clear the redo stack).
    pub fn set_suppress_undo(&mut self, suppress: bool) {
        self.suppress_undo = suppress;
    }

    /// Snapshot the current state to the attached recorder (if any). Must be
    /// called BEFORE the mutation so the snapshot captures the prior value.
    /// Mirrors Flutter's `UndoHistory` listener: every value change records,
    /// uniformly. No-op when no recorder is attached or while `suppress_undo`
    /// is set.
    fn maybe_push_undo(&mut self) {
        if self.suppress_undo {
            return;
        }
        let recorder = match self.undo_recorder.clone() {
            Some(r) => r,
            None => return,
        };
        let snapshot = crate::builtin_plugins::text::controller::TextEditingValue {
            text: self.text(),
            cursor_position: self.cursor_position,
            selection_anchor: self.selection_anchor,
            selection_end: self.selection_end,
        };
        recorder.borrow_mut().push(snapshot);
    }

    /// The full text (all spans joined). Memoized on the content revision —
    /// the join is O(document) and this is called several times per keystroke
    /// and once per painted frame.
    pub fn text(&self) -> String {
        if let Some((rev, text)) = self.cached_text.borrow().as_ref()
            && *rev == self.revision
        {
            return text.clone();
        }
        let joined: String = self.spans.iter().map(|s| s.text.as_str()).collect();
        *self.cached_text.borrow_mut() = Some((self.revision, joined.clone()));
        joined
    }

    pub fn spans(&self) -> &[SpanData] {
        &self.spans
    }

    pub fn set_spans(&mut self, spans: Vec<SpanData>) {
        // Push undo only when the text actually changes — re-tokenization
        // (e.g. live syntax highlighting) passes the same text with new span
        // colors and must NOT create an undo entry.
        let new_text: String = spans.iter().map(|s| s.text.as_str()).collect();
        if new_text != self.text() {
            self.maybe_push_undo();
        }
        // Bump the revision only when the *rendered content* differs: a
        // no-op re-highlight (identical spans) stays free — the element's
        // layout memo keeps its hit and the document is not re-shaped.
        // Clearing an active composition also changes the rendered text.
        let content_changed = spans != self.spans || self.composing_text.is_some();
        self.spans = spans;
        if content_changed {
            self.invalidate();
        }
        self.cursor_position = self.full_len();
        self.selection_anchor = self.cursor_position;
        self.selection_end = self.cursor_position;
        self.composing_text = None;
    }

    /// Replace the spans while preserving the current cursor position and
    /// selection (clamped to the new length). Use this for live
    /// re-tokenization (e.g. syntax highlighting) where `set_spans` would
    /// yank the caret to EOF. Mirrors Flutter's `TextEditingController`,
    /// where selection is part of the `TextEditingValue` and is preserved
    /// across value updates.
    pub fn set_spans_preserve_cursor(&mut self, spans: Vec<SpanData>) {
        let new_text: String = spans.iter().map(|s| s.text.as_str()).collect();
        if new_text != self.text() {
            self.maybe_push_undo();
        }
        let content_changed = spans != self.spans;
        let len = spans.iter().map(|s| s.text.len()).sum();
        self.spans = spans;
        if content_changed {
            self.invalidate();
        }
        self.cursor_position = self.cursor_position.min(len);
        self.selection_anchor = self.selection_anchor.min(len);
        self.selection_end = self.selection_end.min(len);
        // Do NOT clear composing_text — composition can continue across a
        // re-highlight pass.
    }

    pub fn clear(&mut self) {
        if !self.spans.is_empty() {
            self.maybe_push_undo();
        }
        let had_content = !self.spans.is_empty() || self.composing_text.is_some();
        self.spans.clear();
        self.cursor_position = 0;
        self.selection_anchor = 0;
        self.selection_end = 0;
        self.composing_text = None;
        self.composing_start = 0;
        if had_content {
            self.invalidate();
        }
    }

    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    pub fn set_cursor_position(&mut self, pos: usize) {
        self.cursor_position = pos;
    }

    pub fn selection_anchor(&self) -> usize {
        self.selection_anchor
    }

    pub fn selection_end(&self) -> usize {
        self.selection_end
    }

    pub fn has_selection(&self) -> bool {
        self.selection_anchor != self.selection_end
    }

    pub fn selection_range(&self) -> (usize, usize) {
        let (a, b) = (self.selection_anchor, self.selection_end);
        if a <= b { (a, b) } else { (b, a) }
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = self.cursor_position;
        self.selection_end = self.cursor_position;
    }

    pub fn delete_selection(&mut self) {
        if !self.has_selection() {
            return;
        }
        let (start, end) = self.selection_range();
        self.delete_range(start, end);
        self.cursor_position = start;
        self.clear_selection();
    }

    pub fn is_composing(&self) -> bool {
        self.composing_text.is_some()
    }

    pub fn full_len(&self) -> usize {
        self.spans.iter().map(|s| s.text.len()).sum()
    }

    pub fn insert_char_at(&mut self, pos: usize, ch: char) {
        self.insert_at(pos, &ch.to_string());
    }

    pub fn insert_str_at(&mut self, pos: usize, text: &str) {
        self.insert_at(pos, text);
    }

    pub fn composition_display_text(&self) -> String {
        let base = self.text();
        if let Some(ref comp) = self.composing_text {
            let start = self.composing_start.min(base.len());
            format!("{}{}{}", &base[..start], comp, &base[start..])
        } else {
            base
        }
    }

    pub fn start_composition(&mut self) {
        self.composing_text = Some(String::new());
        self.composing_start = self.cursor_position;
        self.invalidate();
    }

    pub fn update_composition(&mut self, text: String) {
        if self.composing_text.is_some() {
            self.composing_text = Some(text);
            self.invalidate();
        }
    }

    pub fn finish_composition(&mut self) -> Option<String> {
        let taken = self.composing_text.take();
        if taken.is_some() {
            self.invalidate();
        }
        taken
    }

    pub fn composing_text(&self) -> Option<&String> {
        self.composing_text.as_ref()
    }

    pub fn composing_start(&self) -> usize {
        self.composing_start
    }

    pub fn set_selection(&mut self, anchor: usize, end: usize) {
        self.selection_anchor = anchor;
        self.selection_end = end;
    }

    pub fn on_input(&self) -> Option<MutationHandle<InputEvent>> {
        self.on_input
    }

    pub fn on_cursor_change(&self) -> Option<MutationHandle<CursorChangeEvent>> {
        self.on_cursor_change
    }

    pub fn on_selection_change(&self) -> Option<MutationHandle<SelectionChangeEvent>> {
        self.on_selection_change
    }

    pub fn on_key_down(&self) -> Option<MutationHandle<KeydownEvent>> {
        self.on_key_down
    }

    pub fn on_key_up(&self) -> Option<MutationHandle<KeyupEvent>> {
        self.on_key_up
    }

    pub fn on_focus(&self) -> Option<MutationHandle<FocusEvent>> {
        self.on_focus
    }

    pub fn on_blur(&self) -> Option<MutationHandle<BlurEvent>> {
        self.on_blur
    }

    pub fn on_composition_start(&self) -> Option<MutationHandle<CompositionStartEvent>> {
        self.on_composition_start
    }

    pub fn on_composition_update(&self) -> Option<MutationHandle<CompositionUpdateEvent>> {
        self.on_composition_update
    }

    pub fn on_composition_end(&self) -> Option<MutationHandle<CompositionEndEvent>> {
        self.on_composition_end
    }

    fn span_index_at(&self, byte_pos: usize) -> (usize, usize) {
        let mut offset = 0;
        for (i, span) in self.spans.iter().enumerate() {
            let end = offset + span.text.len();
            if byte_pos <= end {
                return (i, byte_pos - offset);
            }
            offset = end;
        }
        if self.spans.is_empty() {
            return (0, 0);
        }
        let last = self.spans.len() - 1;
        (last, self.spans[last].text.len())
    }

    fn insert_at(&mut self, byte_pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        self.maybe_push_undo();
        self.invalidate();
        if self.spans.is_empty() {
            self.spans.push(SpanData {
                text: text.to_string(),
                weight: None,
                italic: false,
                underline: false,
                font_size: None,
                color: None,
            });
            return;
        }
        let (idx, local_offset) = self.span_index_at(byte_pos);
        self.spans[idx].text.insert_str(local_offset, text);
    }

    pub fn delete_range(&mut self, start: usize, end: usize) {
        if start >= end || self.spans.is_empty() {
            return;
        }
        let total = self.full_len();
        let end = end.min(total);
        let start = start.min(total);
        if start >= end {
            return;
        }

        self.maybe_push_undo();
        self.invalidate();

        let (start_idx, start_local) = self.span_index_at(start);
        let (end_idx, end_local) = self.span_index_at(end);

        if start_idx == end_idx {
            self.spans[start_idx]
                .text
                .replace_range(start_local..end_local, "");
        } else {
            self.spans[start_idx].text.truncate(start_local);
            self.spans[end_idx].text.replace_range(0..end_local, "");
            for i in (start_idx + 1..end_idx).rev() {
                self.spans.remove(i);
            }
        }

        self.spans.retain(|s| !s.text.is_empty());
        self.merge_adjacent();
    }

    fn merge_adjacent(&mut self) {
        if self.spans.len() <= 1 {
            return;
        }
        let mut i = 0;
        while i < self.spans.len() - 1 {
            let can_merge = {
                let a = &self.spans[i];
                let b = &self.spans[i + 1];
                a.weight == b.weight
                    && a.italic == b.italic
                    && a.underline == b.underline
                    && a.font_size == b.font_size
                    && a.color == b.color
            };
            if can_merge {
                let b_text = self.spans[i + 1].text.clone();
                self.spans[i].text.push_str(&b_text);
                self.spans.remove(i + 1);
            } else {
                i += 1;
            }
        }
    }
}

impl Default for TextEditingController {
    fn default() -> Self {
        Self::new()
    }
}
