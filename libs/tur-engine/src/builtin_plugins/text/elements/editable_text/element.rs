use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::core::render::brush::Color;
use boa_engine::class::Class;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use unicode_segmentation::UnicodeSegmentation;

use crate::builtin_plugins::text::controller::TextEditingController;use crate::builtin_plugins::text::controller::{
    CompositionEndEvent, CompositionStartEvent, CompositionUpdateEvent, CursorChangeEvent,
    InputEvent, SelectionChangeEvent,
};
use crate::builtin_plugins::text::elements::text_shared::span_data::SpanData;

use crate::core::edgy::mutation::{IntoJsArgs, MutationHandle};
use crate::core::edgy::reactive::AnyReadable;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{
    AnyElement, ComposedGestureEvent, ElementOnFocus, ElementOnGesture, ElementOnGestureContext,
    ElementOnIme, ElementOnImeContext, ElementOnKeyboard, ElementOnKeyboardContext, ElementTrace,
    TraceValue,
};
use crate::core::focus::{BlurEvent, FocusEvent, Focusable};
use crate::core::js_runtime::JsProps;
use crate::core::layout::{ElementSubscribe, SubscribeCx};
use crate::core::platform::ImeEvent;
use crate::core::platform::PointerDeviceKind;
use crate::core::platform::key_event::KeydownEvent;
use crate::core::platform::key_event::{KeyEvent, KeyEventType};
use crate::core::scheduler::TaskHandle;
use crate::core::text::text_layout::TextLayoutData;
use crate::core::view::{Lifecycle, SharedViewCx, Val, View, ViewCx, read_atom_raw};

/// Default text color (opaque black) shared by the layout (text fall-back)
/// and render (cursor fall-back) modules.
pub(super) const DEFAULT_TEXT_COLOR: Color = Color::rgb(0, 0, 0);

#[derive(Clone)]
pub struct LineNavInfo {
    line_start_bytes: Vec<usize>,
    line_end_bytes: Vec<usize>,
    /// (byte, x) cursor stops per line — used by ArrowUp/Down to pick the
    /// closest column on the target line.
    line_stops: Vec<Vec<(usize, f32)>>,
    cursor_xy: (f32, f32),
    current_line: usize,
}

impl LineNavInfo {
    fn extract(ld: &TextLayoutData, cursor_byte: usize) -> Self {
        let current_line = ld.line_index_for_byte(cursor_byte);
        let cursor_xy = ld.cursor_xy_at(cursor_byte);

        let line_stops = (0..ld.line_infos.len())
            .map(|i| {
                let info = &ld.line_infos[i];
                let mut stops: Vec<(usize, f32)> =
                    info.stops.iter().map(|s| (s.byte, s.x)).collect();
                // Virtual stop for the end-of-line caret position (after the
                // last glyph) so ArrowUp/Down preserve the column when the
                // cursor sits at a line's right edge.
                stops.push((info.end_byte, info.right_x));
                stops
            })
            .collect();

        LineNavInfo {
            line_start_bytes: (0..ld.line_infos.len())
                .map(|i| ld.line_start_byte(i))
                .collect(),
            line_end_bytes: (0..ld.line_infos.len())
                .map(|i| ld.line_end_byte(i))
                .collect(),
            line_stops,
            cursor_xy,
            current_line,
        }
    }
}

// ---------------------------------------------------------------------------
// EditableTextView — the user's declaration. Pure Rust, no JsValues except
// the opaque `controller` (a TextEditingController class instance).
//
// `controller` is parsed eagerly (not reactive). The text-style props
// (`placeholder`, `color`, `placeholder_color`, `cursor_color`, `font_size`,
// `multiline`) are reactive (`Val<T>`).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct EditableTextView {
    pub(crate) controller: Option<JsObject>,
    pub(crate) controller_atom: Option<AnyReadable>,
    /// Optional `UndoController` for Cmd/Ctrl+Z + Cmd/Ctrl+Shift+Z support.
    /// When `Some`, text-mutating keystrokes push a prior-state snapshot
    /// onto the undo stack, and `handle_key_event` intercepts `"z"` / `"y"`
    /// with the appropriate modifiers.
    pub(crate) undo_controller: Option<JsObject>,
    pub(crate) placeholder: Option<Val<String>>,
    pub(crate) color: Option<Val<Color>>,
    pub(crate) placeholder_color: Option<Val<Color>>,
    pub(crate) cursor_color: Option<Val<Color>>,
    pub(crate) font_size: Option<Val<f64>>,
    pub(crate) font_family: Option<Val<String>>,
    /// Default CSS-style font weight (100–1000). `None` falls back to
    /// parley's default (400). Per-span `weight` overrides this.
    pub(crate) font_weight: Option<Val<f64>>,
    pub(crate) multiline: Option<Val<bool>>,
    /// When `Some(true)`, every character of the value (and any in-progress
    /// IME composition) is rendered as `obscuring_character` — the
    /// controller's stored value stays the real input. Mirrors Flutter's
    /// `obscureText`.
    pub(crate) obscure_text: Option<Val<bool>>,
    /// The mask glyph used when `obscure_text` is active (default `•`).
    pub(crate) obscuring_character: Option<Val<String>>,
    pub(crate) on_context_menu: Option<MutationHandle<ContextMenuEvent>>,
    pub(crate) query_key: Option<Vec<String>>,
}

impl View for EditableTextView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        let mut spec = self.clone();

        if spec.controller.is_none()
            && let Some(readable) = spec.controller_atom
        {
            let js_val = read_atom_raw(cx, readable, boa);
            if let Some(obj) = js_val.as_object()
                && obj.downcast_ref::<TextEditingController>().is_some()
            {
                spec.controller = Some(obj.clone());
            }
        }

        if spec.controller.is_none() {
            let data = TextEditingController::data_constructor(&JsValue::undefined(), &[], boa)
                .expect("failed to construct default TextEditingController");
            let obj = TextEditingController::from_data(data, boa)
                .expect("failed to wrap default TextEditingController");
            spec.controller = Some(obj.upcast().clone());
        }

        // Attach the undo recorder to the controller so every text mutation
        // (keyboard, IME, JS bridge, programmatic setSpans) records to the
        // history stack uniformly — mirroring Flutter's `UndoHistory` listener
        // model. See `TextEditingController::maybe_push_undo`.
        if let Some(undo_obj) = spec.undo_controller.clone()
            && let Some(ctrl_obj) = spec.controller.as_ref()
            && let Some(mut ctrl) = ctrl_obj.downcast_mut::<TextEditingController>()
        {
            ctrl.set_undo_recorder(Some(undo_obj));
        }

        cx.insert_node(
            id,
            AnyElement::with_full_interactivity(EditableTextElement {
                view: spec,
                cached_layout: None,
                resolved_multiline: false,
                resolved_obscured: false,
                resolved_obscuring_char: '\u{2022}',
                painting: EditableTextPainting::default(),
                blink_task: None,
            })
            .with_callbacks()
            .with_cursor_rect::<EditableTextElement>()
            .with_focusable::<EditableTextElement>(),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// EditableTextElement — the built element. Holds its spec plus the runtime
// text-layout cache and the last-resolved `multiline` flag (needed by the
// gesture/keyboard/IME handlers which lack store access — those props are
// refreshed during layout via `LayoutContext::read_val`).
// ---------------------------------------------------------------------------

/// Resolved paint props (filled during layout). Paint reads these directly.
#[derive(Default, Clone)]
pub struct EditableTextPainting {
    pub(crate) color: Option<Color>,
    pub(crate) cursor_color: Option<Color>,
}

pub struct EditableTextElement {
    pub(crate) view: EditableTextView,
    pub(crate) cached_layout: Option<Arc<TextLayoutData>>,
    pub(crate) resolved_multiline: bool,
    /// Last-resolved `obscureText` flag (refreshed during layout). Read by
    /// the keyboard handler (to suppress copy/cut) and the render path (to
    /// skip the composition underline).
    pub(crate) resolved_obscured: bool,
    /// Last-resolved `obscuringCharacter` (default `•`).
    pub(crate) resolved_obscuring_char: char,
    pub(crate) painting: EditableTextPainting,
    /// Handle to the caret-blink task. `Some` while focused (the spawned
    /// loop ticks `need_paint` each half-period); `None` when unfocused.
    /// On blur or element drop, the handle is aborted, which drops the
    /// pending `Sleep` and halts the loop immediately.
    pub(crate) blink_task: Option<TaskHandle>,
}

impl Drop for EditableTextElement {
    fn drop(&mut self) {
        // Abort the spawned blink loop — drops its pending Sleep so the
        // next tick never fires.
        if let Some(h) = self.blink_task.take() {
            h.abort();
        }
    }
}

/// Half-period of the caret blink, in milliseconds. The caret is visible on
/// even half-cycles: `(now_ms / CARET_BLINK_HALF_PERIOD_MS) % 2 == 0`.
pub(crate) const CARET_BLINK_HALF_PERIOD_MS: u64 = 530;

impl crate::core::elements::ElementCursorRect for EditableTextElement {
    fn cursor_rect_relative(&self) -> Option<(f64, f64, f64, f64)> {
        let layout_data = self.cached_layout.as_ref()?;
        let cursor_byte = self.cursor_position();
        let (cursor_x, _) = layout_data.cursor_xy_at(cursor_byte);
        let line_idx = layout_data.line_index_for_byte(cursor_byte);
        let line_info = &layout_data.line_infos[line_idx];
        Some((
            cursor_x as f64,
            line_info.top as f64,
            2.0,
            line_info.height as f64,
        ))
    }
}

impl Focusable for EditableTextElement {
    fn on_focus_mutation(&self) -> Option<MutationHandle<FocusEvent>> {
        self.controller().on_focus()
    }

    fn on_blur_mutation(&self) -> Option<MutationHandle<BlurEvent>> {
        self.controller().on_blur()
    }
}

impl EditableTextElement {
    pub fn controller(&self) -> boa_engine::object::Ref<'_, TextEditingController> {
        self.view
            .controller
            .as_ref()
            .expect("controller is always present")
            .downcast_ref::<TextEditingController>()
            .expect("controller is always a valid TextEditingController")
    }

    pub fn controller_mut(&self) -> boa_engine::object::RefMut<'_, TextEditingController> {
        self.view
            .controller
            .as_ref()
            .expect("controller is always present")
            .downcast_mut::<TextEditingController>()
            .expect("controller is always a valid TextEditingController")
    }

    pub fn undo_controller_mut(
        &self,
    ) -> Option<
        boa_engine::object::RefMut<'_, crate::builtin_plugins::text::controller::UndoController>,
    > {
        self.view
            .undo_controller
            .as_ref()?
            .downcast_mut::<crate::builtin_plugins::text::controller::UndoController>()
    }

    pub fn text(&self) -> String {
        self.controller().text()
    }

    pub fn cursor_position(&self) -> usize {
        self.controller().cursor_position()
    }

    pub fn selection(&self) -> (usize, usize) {
        let c = self.controller();
        (c.selection_anchor(), c.selection_end())
    }

    pub fn composition_display_text(&self) -> String {
        self.controller().composition_display_text()
    }

    /// The text currently being rendered. When `obscureText` is active this is
    /// the masked form (each character → `obscuringCharacter`); the stored
    /// value ([`text`](Self::text)) stays the real input. Exposed for
    /// tests/inspection.
    pub fn displayed_text(&self) -> String {
        match self.build_masked() {
            Some((masked, _, _)) => masked,
            None => self.composition_display_text(),
        }
    }

    /// x of the caret at the given value-byte offset, from the cached layout.
    /// Exposed for tests so they can target a click precisely.
    pub fn cursor_x_at(&self, byte: usize) -> Option<f32> {
        self.cached_layout.as_ref().map(|ld| ld.cursor_x_at(byte))
    }

    /// Build the masked display string (each *grapheme cluster* of the value
    /// — and any in-progress IME composition — replaced by one
    /// `obscuringCharacter`) plus a map from each display char index to the
    /// corresponding byte offset in the controller's real value. Returns
    /// `None` when `obscureText` is off.
    ///
    /// Masking is per grapheme cluster (UAX #29), not per code point, so
    /// multi-code-point graphemes (combining marks, flag emoji, ZWJ sequences,
    /// skin-tone modifiers) collapse to a single bullet — matching the
    /// engine's own grapheme-based cursor/backspace model and Flutter/web
    /// password fields. The map lets the layout speak in *value* byte space
    /// (the same space as the controller's cursor/selection): layout builds
    /// the masked string, then remaps every glyph-stop / line byte offset via
    /// this map, so all cursor/caret/selection/click math works unchanged.
    pub(super) fn build_masked(&self) -> Option<(String, Vec<usize>, usize)> {
        if !self.resolved_obscured {
            return None;
        }
        let mask_char = self.resolved_obscuring_char;
        let mask_len = mask_char.len_utf8();
        let base = self.text();
        let (comp, cs_byte) = {
            let c = self.controller();
            (
                c.composing_text().cloned().unwrap_or_default(),
                c.composing_start().min(base.len()),
            )
        };
        // Display graphemes, in order: base[..cs] (each masked), then the
        // composition (each masked), then base[cs..] (each masked). Each
        // records the value byte a cursor placed there must map to.
        let mut out = String::new();
        let mut map = Vec::new();
        for (i, _g) in base[..cs_byte].grapheme_indices(true) {
            out.push(mask_char);
            map.push(i);
        }
        for _g in comp.grapheme_indices(true) {
            out.push(mask_char);
            map.push(cs_byte);
        }
        for (i, _g) in base[cs_byte..].grapheme_indices(true) {
            out.push(mask_char);
            map.push(cs_byte + i);
        }
        // End-of-string cursor position.
        map.push(base.len());
        Some((out, map, mask_len))
    }

    fn handle_key_event(
        &self,
        key: &str,
        ctrl: bool,
        meta: bool,
        shift: bool,
        nav_info: Option<&LineNavInfo>,
    ) -> (bool, Option<String>) {
        let multiline = self.resolved_multiline;
        let mut c = self.controller_mut();
        let full = c.text();
        let cursor = c.cursor_position();
        let anchor = c.selection_anchor();
        let end = c.selection_end();
        let has_sel = c.has_selection();
        let len = c.full_len();
        let composing = c.is_composing();

        let mut new_cursor = cursor;
        let mut new_anchor = anchor;
        let mut new_end = end;
        let mut new_clipboard_write: Option<String> = None;

        let handled = match key {
            "Backspace" => {
                if has_sel {
                    let (s, e) = if anchor <= end {
                        (anchor, end)
                    } else {
                        (end, anchor)
                    };
                    c.delete_range(s, e);
                    new_cursor = s;
                    new_anchor = s;
                    new_end = s;
                    true
                } else if cursor > 0 {
                    let new_pos = prev_grapheme_boundary(&full, cursor);
                    let del_end = next_grapheme_boundary(&full, new_pos);
                    c.delete_range(new_pos, del_end);
                    new_cursor = new_pos;
                    new_anchor = new_pos;
                    new_end = new_pos;
                    true
                } else {
                    false
                }
            }
            "Delete" => {
                if has_sel {
                    let (s, e) = if anchor <= end {
                        (anchor, end)
                    } else {
                        (end, anchor)
                    };
                    c.delete_range(s, e);
                    new_cursor = s;
                    new_anchor = s;
                    new_end = s;

                    true
                } else if cursor < len {
                    let del_end = next_grapheme_boundary(&full, cursor);
                    c.delete_range(cursor, del_end);
                    new_anchor = cursor;
                    new_end = cursor;

                    true
                } else {
                    false
                }
            }
            "ArrowLeft" => {
                if shift {
                    new_end = prev_grapheme_boundary(&full, end);
                    if !has_sel {
                        new_anchor = cursor;
                    }
                    new_cursor = new_end;
                } else if has_sel {
                    new_cursor = if anchor <= end { anchor } else { end };
                    new_anchor = new_cursor;
                    new_end = new_cursor;
                } else {
                    new_cursor = prev_grapheme_boundary(&full, cursor);
                    new_anchor = new_cursor;
                    new_end = new_cursor;
                }
                true
            }
            "ArrowRight" => {
                if shift {
                    new_end = next_grapheme_boundary(&full, end);
                    if !has_sel {
                        new_anchor = cursor;
                    }
                    new_cursor = new_end;
                } else if has_sel {
                    new_cursor = if anchor <= end { end } else { anchor };
                    new_anchor = new_cursor;
                    new_end = new_cursor;
                } else {
                    new_cursor = next_grapheme_boundary(&full, cursor);
                    new_anchor = new_cursor;
                    new_end = new_cursor;
                }
                true
            }
            "ArrowUp" if multiline => {
                if let Some(info) = nav_info {
                    let target_byte = compute_vertical_target(info, -1);
                    if shift {
                        if !has_sel {
                            new_anchor = cursor;
                        }
                        new_end = target_byte;
                        new_cursor = target_byte;
                    } else {
                        new_cursor = target_byte;
                        new_anchor = target_byte;
                        new_end = target_byte;
                    }
                    true
                } else {
                    false
                }
            }
            "ArrowDown" if multiline => {
                if let Some(info) = nav_info {
                    let target_byte = compute_vertical_target(info, 1);
                    if shift {
                        if !has_sel {
                            new_anchor = cursor;
                        }
                        new_end = target_byte;
                        new_cursor = target_byte;
                    } else {
                        new_cursor = target_byte;
                        new_anchor = target_byte;
                        new_end = target_byte;
                    }
                    true
                } else {
                    false
                }
            }
            "Home" => {
                let target = if multiline {
                    if let Some(info) = nav_info {
                        info.line_start_bytes[info.current_line]
                    } else {
                        return (false, None);
                    }
                } else {
                    0
                };
                if shift {
                    if !has_sel {
                        new_anchor = cursor;
                    }
                    new_end = target;
                    new_cursor = target;
                } else {
                    new_cursor = target;
                    new_anchor = target;
                    new_end = target;
                }
                true
            }
            "End" => {
                let target = if multiline {
                    if let Some(info) = nav_info {
                        info.line_end_bytes[info.current_line]
                    } else {
                        return (false, None);
                    }
                } else {
                    len
                };
                if shift {
                    if !has_sel {
                        new_anchor = cursor;
                    }
                    new_end = target;
                    new_cursor = target;
                } else {
                    new_cursor = target;
                    new_anchor = target;
                    new_end = target;
                }
                true
            }
            "a" if ctrl || meta => {
                new_anchor = 0;
                new_end = len;
                new_cursor = len;
                true
            }
            "c" if ctrl || meta => {
                // Copy: fire the host clipboard bridge with the selected text.
                // The buffer is unchanged; only the system clipboard is written.
                // Suppressed in password mode so the real value can't be
                // exfiltrated via the clipboard (matches Flutter).
                if !self.resolved_obscured && has_sel {
                    let (s, e) = if anchor <= end {
                        (anchor, end)
                    } else {
                        (end, anchor)
                    };
                    new_clipboard_write = Some(full[s..e].to_string());
                }
                true
            }
            "x" if ctrl || meta => {
                // Cut: write selection to clipboard then delete it. Suppressed
                // in password mode (same reason as copy).
                if !self.resolved_obscured && has_sel {
                    let (s, e) = if anchor <= end {
                        (anchor, end)
                    } else {
                        (end, anchor)
                    };
                    new_clipboard_write = Some(full[s..e].to_string());
                    c.delete_range(s, e);
                    new_cursor = s;
                    new_anchor = s;
                    new_end = s;
                }
                true
            }
            "v" if ctrl || meta => {
                // Paste: the browser fires a `paste` event on the hidden
                // textarea when the user presses Cmd+V; the wasm layer
                // forwards the clipboard text as a ClipboardPlatformPasteEvent
                // (PlatformEvent::Custom), which tur-clipboard's
                // ClipboardPlatformSubsystem re-emits as a ClipboardPasteEvent
                // (AppEvent::Custom). Here we just mark the key as handled so
                // no fallback runs.
                true
            }
            "z" if (ctrl || meta) && self.view.undo_controller.is_some() => {
                // Undo (no shift) / Redo (with shift). The undo controller
                // owns the history stacks; we feed it the controller's
                // current value (already captured above as `full`/`cursor`/
                // `anchor`/`end`) and apply the restored value in-place.
                use crate::builtin_plugins::text::controller::TextEditingValue;
                let current = TextEditingValue {
                    text: full.clone(),
                    cursor_position: cursor,
                    selection_anchor: anchor,
                    selection_end: end,
                };
                let restored = if shift {
                    self.undo_controller_mut().and_then(|mut u| u.redo(current))
                } else {
                    self.undo_controller_mut().and_then(|mut u| u.undo(current))
                };
                if let Some(value) = restored {
                    // Suppress the recorder while applying the restored
                    // value — otherwise `set_spans_preserve_cursor` would
                    // push the current state and clear the redo stack.
                    c.set_suppress_undo(true);
                    c.set_spans_preserve_cursor(vec![SpanData {
                        text: value.text,
                        weight: None,
                        italic: false,
                        underline: false,
                        font_size: None,
                        color: None,
                    }]);
                    c.set_suppress_undo(false);
                    new_cursor = value.cursor_position;
                    new_anchor = value.selection_anchor;
                    new_end = value.selection_end;
                }
                true
            }
            "y" if ctrl && self.view.undo_controller.is_some() => {
                // Ctrl+Y redo (Windows convention) — mirror of Cmd+Shift+Z.
                use crate::builtin_plugins::text::controller::TextEditingValue;
                let current = TextEditingValue {
                    text: full.clone(),
                    cursor_position: cursor,
                    selection_anchor: anchor,
                    selection_end: end,
                };
                let restored = self.undo_controller_mut().and_then(|mut u| u.redo(current));
                if let Some(value) = restored {
                    c.set_suppress_undo(true);
                    c.set_spans_preserve_cursor(vec![SpanData {
                        text: value.text,
                        weight: None,
                        italic: false,
                        underline: false,
                        font_size: None,
                        color: None,
                    }]);
                    c.set_suppress_undo(false);
                    new_cursor = value.cursor_position;
                    new_anchor = value.selection_anchor;
                    new_end = value.selection_end;
                }
                true
            }
            "Enter" => {
                if multiline {
                    if has_sel {
                        let (s, e) = if anchor <= end {
                            (anchor, end)
                        } else {
                            (end, anchor)
                        };
                        c.delete_range(s, e);
                        new_cursor = s;
                    }
                    c.insert_char_at(new_cursor, '\n');
                    new_cursor += '\n'.len_utf8();
                    new_anchor = new_cursor;
                    new_end = new_cursor;

                    true
                } else {
                    true
                }
            }
            _ => {
                if key.len() == 1 && !ctrl && !meta && !composing {
                    let ch = key.chars().next().unwrap();
                    if has_sel {
                        let (s, e) = if anchor <= end {
                            (anchor, end)
                        } else {
                            (end, anchor)
                        };
                        c.delete_range(s, e);
                        new_cursor = s;
                    }
                    c.insert_char_at(new_cursor, ch);
                    new_cursor += ch.len_utf8();
                    new_anchor = new_cursor;
                    new_end = new_cursor;

                    true
                } else {
                    false
                }
            }
        };

        if handled {
            c.set_cursor_position(new_cursor);
            c.set_selection(new_anchor, new_end);
        }

        (handled, new_clipboard_write)
    }

    fn char_index_at(&self, local_position: &crate::core::layout::Offset) -> usize {
        // When the buffer is empty and no IME composition is active, the
        // cached layout was built from the placeholder text — clicking
        // anywhere inside it must still map to byte 0 so the caret lands
        // at the first position and the placeholder cannot be selected.
        let c = self.controller();
        if c.text().is_empty() && !c.is_composing() {
            return 0;
        }
        drop(c);
        self.cached_layout
            .as_ref()
            .map(|ld| {
                if self.resolved_multiline {
                    ld.byte_index_at_xy(local_position.x as f32, local_position.y as f32)
                } else {
                    ld.byte_index_at_x(local_position.x as f32)
                }
            })
            .unwrap_or(0)
    }
}

fn compute_vertical_target(info: &LineNavInfo, direction: i32) -> usize {
    let current_line = info.current_line;
    let cursor_x = info.cursor_xy.0;
    let num_lines = info.line_start_bytes.len();

    let target_line = if direction < 0 {
        current_line.saturating_sub(1)
    } else {
        (current_line + 1).min(num_lines - 1)
    };

    if target_line == current_line {
        // Already on the top/bottom line: ArrowUp on the first line moves to
        // the line start, ArrowDown on the last line moves to the line end
        // (matches typical editor behavior).
        return if direction < 0 {
            info.line_start_bytes[current_line]
        } else {
            info.line_end_bytes[current_line]
        };
    }

    let stops = &info.line_stops[target_line];
    if stops.is_empty() {
        return info.line_start_bytes[target_line];
    }

    // Pick the stop closest to the current cursor x. On a tie prefer the
    // earlier (leftward) stop so repeated ArrowDown doesn't drift right.
    let mut best_byte = stops.last().unwrap().0;
    let mut best_dist = f32::MAX;
    for &(byte, x) in stops.iter() {
        let dist = (x - cursor_x).abs();
        if dist < best_dist {
            best_dist = dist;
            best_byte = byte;
        }
    }
    best_byte
}

fn prev_grapheme_boundary(s: &str, byte_pos: usize) -> usize {
    let mut prev = 0;
    for (i, _) in s.grapheme_indices(true) {
        if i >= byte_pos {
            break;
        }
        prev = i;
    }
    prev
}

fn next_grapheme_boundary(s: &str, byte_pos: usize) -> usize {
    for (i, _) in s.grapheme_indices(true) {
        if i > byte_pos {
            return i;
        }
    }
    s.len()
}

// ---------------------------------------------------------------------------
// ContextMenuEvent — JS callback argument for the right-click menu trigger.
// Carries both local (element-relative) and global (canvas-relative) coords.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ContextMenuEvent {
    local: crate::core::layout::Offset,
    global: crate::core::layout::Offset,
}

impl IntoJsArgs for ContextMenuEvent {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue> {
        use boa_engine::js_string;
        use boa_engine::object::JsObject;

        fn make_point(ctx: &mut Context, x: f64, y: f64) -> JsObject {
            let obj = JsObject::with_object_proto(ctx.intrinsics());
            let _ = obj.create_data_property(js_string!("x"), JsValue::from(x), ctx);
            let _ = obj.create_data_property(js_string!("y"), JsValue::from(y), ctx);
            obj
        }
        fn make_event(ctx: &mut Context, local: JsObject, global: JsObject) -> JsObject {
            let obj = JsObject::with_object_proto(ctx.intrinsics());
            let _ = obj.create_data_property(js_string!("local"), JsValue::from(local), ctx);
            let _ = obj.create_data_property(js_string!("global"), JsValue::from(global), ctx);
            obj
        }

        let local = make_point(ctx, self.local.x, self.local.y);
        let global = make_point(ctx, self.global.x, self.global.y);
        let event = make_event(ctx, local, global);
        vec![JsValue::from(event)]
    }
}

impl Lifecycle for EditableTextElement {
    fn on_focus_changed(&mut self, focused: bool, cx: &mut SharedViewCx, _boa: &mut Context) {
        if focused {
            let worker_sched = cx.js_ctx().worker_sched().clone();
            let need_paint = cx.js_ctx().need_paint.clone();
            let worker_sched_for_loop = worker_sched.clone();
            // Spawn the blink loop; abort() on blur/drop drops the pending
            // Sleep + halts the loop immediately (no per-tick flag).
            let fut: Pin<Box<dyn std::future::Future<Output = ()> + 'static>> =
                Box::pin(async move {
                    loop {
                        worker_sched_for_loop
                            .sleep(Duration::from_millis(CARET_BLINK_HALF_PERIOD_MS))
                            .await;
                        need_paint.set(true);
                    }
                });
            self.blink_task = Some(worker_sched.spawn_local(fut));
        } else {
            // Abort the spawned loop — drops its pending Sleep.
            if let Some(h) = self.blink_task.take() {
                h.abort();
            }
        }
    }
}

impl ElementSubscribe for EditableTextElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        let c = &self.view;
        if let Some(v) = c.multiline.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.obscure_text.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.obscuring_character.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.font_size.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.font_family.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.font_weight.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.placeholder.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.color.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.placeholder_color.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = c.cursor_color.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl ElementTrace for EditableTextElement {
    fn trace_label(&self) -> String {
        let text = self.text();
        if text.is_empty() {
            String::new()
        } else {
            let t: String = text.chars().take(20).collect();
            format!("\"{}\"", t)
        }
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let c = self.controller();
        vec![
            ("textLength", TraceValue::Num(c.full_len() as f64)),
            ("cursor", TraceValue::Num(c.cursor_position() as f64)),
            ("anchor", TraceValue::Num(c.selection_anchor() as f64)),
            ("selectionEnd", TraceValue::Num(c.selection_end() as f64)),
        ]
    }

    fn trace_layout_extra(&self) -> Vec<(&'static str, TraceValue)> {
        self.cached_layout
            .as_ref()
            .map(|ld| {
                vec![
                    ("numLines", TraceValue::Num(ld.line_infos.len() as f64)),
                    ("layoutWidth", TraceValue::Num(ld._width as f64)),
                    ("layoutHeight", TraceValue::Num(ld._height as f64)),
                ]
            })
            .unwrap_or_default()
    }
}

impl ElementOnFocus for EditableTextElement {}

impl ElementOnGesture for EditableTextElement {
    fn accepts_device(&self, device: PointerDeviceKind) -> bool {
        // Touch drags should scroll the enclosing ScrollView, not select
        // text. Reject touch so the touch-slop probe falls through to the
        // nearest scroll-capable ancestor. Mouse is accepted for selection
        // and caret placement. (Tap-to-focus still works: a touch tap is
        // funneled through the mouse path as a `PointerDown`/`Click`, which
        // dispatch unconditionally — `accepts_device` only gates the drag-
        // claim probe.)
        matches!(device, PointerDeviceKind::Mouse)
    }

    fn on_gesture_event(&mut self, cx: &mut ElementOnGestureContext, event: &ComposedGestureEvent) {
        match event {
            ComposedGestureEvent::PointerDoubleDown { local, .. } => {
                cx.request_own_focus();
                let byte_pos = self.char_index_at(local);
                // Word selection: expand left + right to the nearest word
                // boundary (sequence of word chars: alphanumerics or
                // underscore, matching most editors' Ctrl+Arrow behaviour).
                // Clicking past end-of-text selects the final word.
                let text = self.controller().text();
                let anchor = word_start(&text, byte_pos);
                let end = word_end(&text, byte_pos);
                let mut c = self.controller_mut();
                c.set_cursor_position(end);
                c.set_selection(anchor, end);
                drop(c);
                cx.request_paint();
            }
            ComposedGestureEvent::PointerTripleDown { local, .. } => {
                cx.request_own_focus();
                let byte_pos = self.char_index_at(local);
                // Line selection: look up the visual line containing the
                // click and select from its start to its end byte. If the
                // layout isn't ready yet, fall back to a caret.
                if let Some(layout) = self.cached_layout.as_ref() {
                    let line = layout.line_index_for_byte(byte_pos);
                    let start = layout.line_start_byte(line);
                    let end = layout.line_end_byte(line);
                    let mut c = self.controller_mut();
                    c.set_cursor_position(end);
                    c.set_selection(start, end);
                    drop(c);
                    cx.request_paint();
                } else {
                    let mut c = self.controller_mut();
                    c.set_cursor_position(byte_pos);
                    c.set_selection(byte_pos, byte_pos);
                    drop(c);
                    cx.request_paint();
                }
            }
            ComposedGestureEvent::PointerDown { local, button, .. } => {
                cx.request_own_focus();
                let byte_pos = self.char_index_at(local);

                // Native-OS selection semantics on right-click:
                //   - If there is an active selection AND the click lands
                //     inside it (inclusive), preserve the selection so the
                //     context-menu's Cut/Copy operate on it (matches native
                //     text fields, browsers, etc.).
                //   - Otherwise (left click, middle click, or right-click
                //     outside the selection), move the caret to the click
                //     position and collapse the selection.
                let preserve = *button == crate::core::layout::MouseButton::Right
                    && self.controller().has_selection()
                    && {
                        let (a, b) = self.controller().selection_range();
                        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                        byte_pos >= lo && byte_pos <= hi
                    };
                if !preserve {
                    let mut c = self.controller_mut();
                    c.set_cursor_position(byte_pos);
                    c.set_selection(byte_pos, byte_pos);
                    drop(c);
                    cx.request_paint();
                }
            }
            ComposedGestureEvent::PointerMove { local, .. } => {
                let byte_pos = self.char_index_at(local);
                let mut c = self.controller_mut();
                let anchor = c.selection_anchor();
                let sel_end = c.selection_end();
                if byte_pos != sel_end {
                    c.set_selection(anchor, byte_pos);
                    c.set_cursor_position(byte_pos);
                    drop(c);
                    cx.request_paint();
                }
            }
            ComposedGestureEvent::PointerUp { .. } => {}
            ComposedGestureEvent::Click { .. } => {}
            ComposedGestureEvent::ContextMenu { local, global, .. } => {
                if let Some(m) = self.view.on_context_menu {
                    cx.push_event(
                        m,
                        ContextMenuEvent {
                            local: *local,
                            global: *global,
                        },
                    );
                }
            }
        }
    }
}

impl ElementOnKeyboard for EditableTextElement {
    fn on_keyboard_event(&mut self, cx: &mut ElementOnKeyboardContext, event: &KeyEvent) {
        if event.event_type != KeyEventType::Down {
            return;
        }

        // Dispatch the controller's `onKeyDown` listener for every keydown
        // (e.g. Cmd+S shortcuts). The text-mutation callbacks below only fire
        // when the buffer changes, so this is dispatched unconditionally.
        if let Some(m) = self.controller().on_key_down() {
            cx.push_event(
                m,
                KeydownEvent {
                    key: event.key.clone(),
                    code: event.code.clone(),
                    modifiers: event.modifiers,
                },
            );
        }

        let (prev_text, prev_cursor, prev_anchor, prev_end) = {
            let c = self.controller();
            (
                c.text(),
                c.cursor_position(),
                c.selection_anchor(),
                c.selection_end(),
            )
        };
        let cursor_byte = prev_cursor;

        let nav_info = self
            .cached_layout
            .as_ref()
            .map(|ld| LineNavInfo::extract(ld, cursor_byte));

        let (changed, clipboard_write) = self.handle_key_event(
            &event.key,
            event.modifiers.ctrl,
            event.modifiers.meta,
            event.modifiers.shift,
            nav_info.as_ref(),
        );

        if let Some(text) = clipboard_write {
            crate::builtin_plugins::clipboard::push_write(cx.app_event_queue(), text);
        }

        if changed {
            cx.request_paint();

            let c = self.controller();
            let new_text = c.text();
            if new_text != prev_text {
                // Undo history is recorded inside the controller's mutating
                // methods now (see `TextEditingController::maybe_push_undo`),
                // so there's nothing to push here — every mutation path
                // (keyboard, IME, JS bridge, programmatic) records uniformly.
                let enter = event.key == "Enter" && !self.resolved_multiline;
                if let Some(m) = c.on_input() {
                    cx.push_event(
                        m,
                        InputEvent {
                            value: new_text,
                            enter,
                        },
                    );
                }
            }
            let cursor = c.cursor_position();
            if cursor != prev_cursor
                && let Some(m) = c.on_cursor_change()
            {
                cx.push_event(m, CursorChangeEvent { position: cursor });
            }
            let anchor = c.selection_anchor();
            let end = c.selection_end();
            if (anchor != prev_anchor || end != prev_end)
                && let Some(m) = c.on_selection_change()
            {
                cx.push_event(m, SelectionChangeEvent { anchor, end });
            }
        }
    }
}

impl ElementOnIme for EditableTextElement {
    fn on_ime_event(&mut self, cx: &mut ElementOnImeContext, event: &ImeEvent) {
        match event {
            ImeEvent::CompositionStart => {
                self.controller_mut().start_composition();
                if let Some(m) = self.controller().on_composition_start() {
                    cx.push_event(m, CompositionStartEvent);
                }
                cx.request_paint();
            }
            ImeEvent::CompositionUpdate { text, .. } => {
                self.controller_mut().update_composition(text.clone());
                if let Some(m) = self.controller().on_composition_update() {
                    cx.push_event(m, CompositionUpdateEvent { text: text.clone() });
                }
                cx.request_paint();
            }
            ImeEvent::CompositionEnd { text } => {
                let mut c = self.controller_mut();
                if c.finish_composition().is_some() {
                    let start = c.composing_start().min(c.full_len());
                    c.insert_str_at(start, text);
                    c.set_cursor_position(start + text.len());
                    c.clear_selection();

                    let new_text = c.text();
                    let cursor = c.cursor_position();
                    let m_end = c.on_composition_end();
                    let m_input = c.on_input();
                    let m_cursor = c.on_cursor_change();
                    let text_end = text.clone();
                    drop(c);
                    if let Some(m) = m_end {
                        cx.push_event(m, CompositionEndEvent { text: text_end });
                    }
                    if let Some(m) = m_input {
                        cx.push_event(
                            m,
                            InputEvent {
                                value: new_text,
                                enter: false,
                            },
                        );
                    }
                    if let Some(m) = m_cursor {
                        cx.push_event(m, CursorChangeEvent { position: cursor });
                    }
                    cx.request_paint();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Factory helpers — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl EditableTextView {
    /// Build an `EditableTextView` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        EditableTextView {
            controller: p.opaque::<TextEditingController>("controller"),
            controller_atom: p.readable("controller"),
            undo_controller: p.opaque::<crate::builtin_plugins::text::controller::UndoController>(
                "undoController",
            ),
            placeholder: p.val::<String>("placeholder"),
            color: p.val::<Color>("color"),
            placeholder_color: p.val::<Color>("placeholderColor"),
            cursor_color: p.val::<Color>("cursorColor"),
            font_size: p.val::<f64>("fontSize"),
            font_family: p.val::<String>("fontFamily"),
            font_weight: p.val::<f64>("fontWeight"),
            multiline: p.val::<bool>("multiline"),
            obscure_text: p.val::<bool>("obscureText"),
            obscuring_character: p.val::<String>("obscuringCharacter"),
            on_context_menu: p.mutation::<ContextMenuEvent>("onContextMenu"),
            query_key: p.query_key("queryKey"),
        }
    }
}

// ---------------------------------------------------------------------------
// Word-boundary helpers for double-click selection. Operate on UTF-8 byte
// offsets to match the controller's `set_selection(anchor, end)` convention.
// Use `unicode_segmentation::UnicodeSegmentation` (already a workspace dep)
// for UAX#29-correct word boundaries — this handles punctuation, digits,
// and non-ASCII letters the same way most editors do.
// ---------------------------------------------------------------------------

/// Return the `(start, end)` byte offsets of the word containing `byte_pos`.
/// "Word" follows UAX#29 word boundaries: a maximal run of alphanumeric
/// (Unicode) chars or `_`. If `byte_pos` lands on a non-word char (space,
/// punctuation), the previous word is returned; if `byte_pos` is past
/// end-of-text, the last word is returned. Returns `(byte_pos, byte_pos)`
/// when no word exists in the text.
fn word_range_at(text: &str, byte_pos: usize) -> (usize, usize) {
    use unicode_segmentation::UnicodeSegmentation;
    let safe_pos = byte_pos.min(text.len());
    let mut last_word: Option<(usize, usize)> = None;
    for (start, segment) in text.split_word_bound_indices() {
        let end = start + segment.len();
        let is_word = segment.chars().any(|c| c.is_alphanumeric() || c == '_');
        if !is_word {
            continue;
        }
        // Position inside this segment (inclusive of the end boundary so a
        // click just past the last word char still selects the word).
        if safe_pos >= start && safe_pos <= end {
            return (start, end);
        }
        last_word = Some((start, end));
    }
    last_word.unwrap_or((safe_pos, safe_pos))
}

fn word_start(text: &str, byte_pos: usize) -> usize {
    word_range_at(text, byte_pos).0
}

fn word_end(text: &str, byte_pos: usize) -> usize {
    word_range_at(text, byte_pos).1
}
