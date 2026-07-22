pub mod events;
pub mod span_data;
pub mod undo_controller;

pub use events::*;
pub use span_data::SpanData;
pub use undo_controller::{TextEditingValue, UndoController};

// ---------------------------------------------------------------------------
// TextEditingController (inlined from controller.rs to avoid module_inception).
// ---------------------------------------------------------------------------

use boa_engine::class::{Class, ClassBuilder};
use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::js_runtime::BoaOpaque;
use crate::core::js_runtime::TurJsContext;
use crate::core::js_runtime::TurNodeHandle;
use crate::core::edgy::mutation::{extract_mutation_from_opts, MutationHandle};
use crate::core::focus::{BlurEvent, FocusEvent};
use crate::core::platform::key_event::{KeydownEvent, KeyupEvent};

#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct TextEditingController {
    spans: Vec<SpanData>,
    cursor_position: usize,
    selection_anchor: usize,
    selection_end: usize,
    composing_text: Option<String>,
    composing_start: usize,
    handle: Option<JsObject>,
    /// Back-reference to the `UndoController` attached via `Input`'s
    /// `undoController` prop. When `Some`, every text-mutating method pushes
    /// a snapshot of the *current* state to the recorder BEFORE mutating —
    /// mirroring Flutter's `UndoHistory` listener model where recording is a
    /// side effect of the controller's value setter, not a per-call-site
    /// concern. Note: a controller shared across multiple `Input`
    /// elements will have its recorder overwritten by whichever element
    /// attaches last (the demo uses a single editor, so this is fine).
    undo_recorder: Option<JsObject>,
    /// Transient flag set by the undo/redo keystroke arms while they apply a
    /// restored value via `set_spans_preserve_cursor`. Prevents the
    /// restoration from pushing the current state and clearing the redo
    /// stack. Single-threaded (boa), no re-entrancy across the JS boundary.
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
            handle: None,
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

    /// Attach an `UndoController` recorder. Called by `Input`'s element
    /// builder when the `undoController` prop is present, so the controller's
    /// text-mutating methods can snapshot to the history stack uniformly —
    /// regardless of whether the mutation originated from a keystroke, IME,
    /// JS bridge, or programmatic `setSpans`.
    pub fn set_undo_recorder(&mut self, recorder: Option<JsObject>) {
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
        if let Some(mut undo) = recorder.downcast_mut::<crate::builtin_plugins::text::controller::UndoController>() {
            undo.push(snapshot);
        }
    }

    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
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
        self.spans = spans;
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
        let len = spans.iter().map(|s| s.text.len()).sum();
        self.spans = spans;
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
        self.spans.clear();
        self.cursor_position = 0;
        self.selection_anchor = 0;
        self.selection_end = 0;
        self.composing_text = None;
        self.composing_start = 0;
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
        if !self.has_selection() { return; }
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
    }

    pub fn update_composition(&mut self, text: String) {
        if self.composing_text.is_some() {
            self.composing_text = Some(text);
        }
    }

    pub fn finish_composition(&mut self) -> Option<String> {
        self.composing_text.take()
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
        if self.spans.is_empty() {
            self.spans.push(SpanData {
                text: text.to_string(),
                bold: false,
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

        let (start_idx, start_local) = self.span_index_at(start);
        let (end_idx, end_local) = self.span_index_at(end);

        if start_idx == end_idx {
            self.spans[start_idx].text.replace_range(start_local..end_local, "");
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
                a.bold == b.bold
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

macro_rules! controller_getter {
    ($class:expr, $name:expr, $body:expr) => {
        let getter = NativeFunction::from_fn_ptr($body)
            .to_js_function($class.context().realm());
        $class.accessor(js_string!($name), Some(getter), None, Attribute::default());
    };
}

impl Class for TextEditingController {
    const NAME: &'static str = "TextEditingController";
    const LENGTH: usize = 1;

    fn data_constructor(
        _new_target: &JsValue,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<Self> {
        let mut ctrl = Self::new();
        if let Some(opts) = args.get_or_undefined(0).as_object() {
            ctrl.on_input = extract_mutation_from_opts(&opts, "onInput", ctx);
            ctrl.on_cursor_change = extract_mutation_from_opts(&opts, "onCursorChange", ctx);
            ctrl.on_selection_change = extract_mutation_from_opts(&opts, "onSelectionChange", ctx);
            ctrl.on_key_down = extract_mutation_from_opts(&opts, "onKeyDown", ctx);
            ctrl.on_key_up = extract_mutation_from_opts(&opts, "onKeyUp", ctx);
            ctrl.on_focus = extract_mutation_from_opts(&opts, "onFocus", ctx);
            ctrl.on_blur = extract_mutation_from_opts(&opts, "onBlur", ctx);
            ctrl.on_composition_start = extract_mutation_from_opts(&opts, "onCompositionStart", ctx);
            ctrl.on_composition_update = extract_mutation_from_opts(&opts, "onCompositionUpdate", ctx);
            ctrl.on_composition_end = extract_mutation_from_opts(&opts, "onCompositionEnd", ctx);

            // Optional initial text — pre-fills the controller so an input
            // that mounts it shows a value other than empty without needing
            // a separate `setSpans` call after mount.
            if let Some(initial) = opts
                .get(js_string!("initialText"), ctx)
                .ok()
                .and_then(|v| v.as_string().map(|s| s.to_std_string_escaped()))
                && !initial.is_empty() {
                    ctrl.set_spans(vec![crate::builtin_plugins::text::controller::span_data::SpanData {
                        text: initial,
                        bold: false,
                        italic: false,
                        underline: false,
                        font_size: None,
                        color: None,
                    }]);
                }
        }
        Ok(ctrl)
    }

    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
        controller_getter!(class, "text", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj.downcast_ref::<TextEditingController>().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            Ok(JsValue::from(js_string!(ctrl.text())))
        });

        controller_getter!(class, "cursorPosition", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj.downcast_ref::<TextEditingController>().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            Ok(JsValue::from(ctrl.cursor_position as f64))
        });

        controller_getter!(class, "selectionAnchor", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj.downcast_ref::<TextEditingController>().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            Ok(JsValue::from(ctrl.selection_anchor as f64))
        });

        controller_getter!(class, "selectionEnd", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj.downcast_ref::<TextEditingController>().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            Ok(JsValue::from(ctrl.selection_end as f64))
        });

        controller_getter!(class, "selectedText", |this, _, _ctx| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj.downcast_ref::<TextEditingController>().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            if !ctrl.has_selection() {
                return Ok(JsValue::from(js_string!("")));
            }
            let full = ctrl.text();
            let (start, end) = ctrl.selection_range();
            let slice = &full[start..end];
            Ok(JsValue::from(js_string!(slice)))
        });

        class.method(
            js_string!("setSpans"),
            1,
            NativeFunction::from_fn_ptr(|this, args, ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj.downcast_mut::<TextEditingController>().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let spans = crate::builtin_plugins::text::controller::span_data::extract_spans_from_js(
                    args.get_or_undefined(0),
                    ctx,
                );
                ctrl.set_spans(spans);
                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("setSpansPreserveCursor"),
            1,
            NativeFunction::from_fn_ptr(|this, args, ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj.downcast_mut::<TextEditingController>().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let spans = crate::builtin_plugins::text::controller::span_data::extract_spans_from_js(
                    args.get_or_undefined(0),
                    ctx,
                );
                ctrl.set_spans_preserve_cursor(spans);
                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("clear"),
            0,
            NativeFunction::from_fn_ptr(|this, _, _| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj.downcast_mut::<TextEditingController>().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                ctrl.clear();
                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("setSelection"),
            2,
            NativeFunction::from_fn_ptr(|this, args, ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj.downcast_mut::<TextEditingController>().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let anchor = args.get_or_undefined(0).to_number(ctx)? as usize;
                let end = args.get_or_undefined(1).to_number(ctx)? as usize;
                ctrl.set_selection(anchor, end);
                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("insertText"),
            1,
            NativeFunction::from_fn_ptr(|this, args, _ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj.downcast_mut::<TextEditingController>().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let text = args
                    .get_or_undefined(0)
                    .as_string()
                    .map(|s| s.to_std_string_escaped())
                    .unwrap_or_default();
                // Replace any existing selection, otherwise insert at the cursor.
                if ctrl.has_selection() {
                    let (start, _end) = ctrl.selection_range();
                    ctrl.delete_selection();
                    ctrl.insert_str_at(start, &text);
                    let new_cursor = start + text.len();
                    ctrl.set_cursor_position(new_cursor);
                    ctrl.set_selection(new_cursor, new_cursor);
                } else {
                    let pos = ctrl.cursor_position();
                    ctrl.insert_str_at(pos, &text);
                    let new_cursor = pos + text.len();
                    ctrl.set_cursor_position(new_cursor);
                    ctrl.set_selection(new_cursor, new_cursor);
                }
                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("deleteSelection"),
            0,
            NativeFunction::from_fn_ptr(|this, _args, _ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj.downcast_mut::<TextEditingController>().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                ctrl.delete_selection();
                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("_attach"),
            1,
            NativeFunction::from_fn_ptr(|this, args, _| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj.downcast_mut::<TextEditingController>().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                if let Some(handle_obj) = args.get_or_undefined(0).as_object()
                    && BoaOpaque::<TurNodeHandle>::wrap(&handle_obj).is_some() {
                        ctrl.handle = Some(handle_obj.clone());
                    }
                Ok(JsValue::undefined())
            }),
        );

        // Explicit JS-side recorder attachment. `Input` attaches
        // automatically from the `undoController` prop, so this is only needed
        // when a controller is mutated outside an Input binding.
        class.method(
            js_string!("setUndoController"),
            1,
            NativeFunction::from_fn_ptr(|this, args, _ctx| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj.downcast_mut::<TextEditingController>().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let recorder = match args.get_or_undefined(0).as_object() {
                    Some(o) if o.downcast_ref::<crate::builtin_plugins::text::controller::UndoController>().is_some() => {
                        Some(o.clone())
                    }
                    _ => None,
                };
                ctrl.set_undo_recorder(recorder);
                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("requestFocus"),
            0,
            NativeFunction::from_fn_ptr(|this, args, _| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let ctrl = obj.downcast_ref::<TextEditingController>().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let Some(ref handle_obj) = ctrl.handle else {
                    return Ok(JsValue::undefined());
                };
                let handle_ref = BoaOpaque::<TurNodeHandle>::wrap(handle_obj).ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid handle")
                })?;
                let node_id = handle_ref.id;

                let js_ctx_obj = args.get_or_undefined(0).as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("expected __ctx")
                })?;
                let js_ctx = BoaOpaque::<TurJsContext>::wrap(&js_ctx_obj).ok_or_else(|| {
                    JsNativeError::typ().with_message("expected __ctx")
                })?;
                let mut focus = js_ctx.focus_manager.borrow_mut();
                focus.set_focus(node_id);
                Ok(JsValue::undefined())
            }),
        );

        Ok(())
    }
}
