use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsString, JsValue};
use tur_shared::{Color, Offset};
use unicode_segmentation::UnicodeSegmentation;

use crate::core::bridge::color::extract_color;
use crate::core::elements::{
    ComposedGestureEvent, ElementJsCallbackEmitter, ElementOnFocus, ElementOnGesture,
    ElementOnGestureContext, ElementOnIme, ElementOnImeContext, ElementOnKeyboard,
    ElementOnKeyboardContext, ElementOnUpdate, ElementTrace,
};
use crate::core::event::AppImeEvent;
use crate::core::js_command::{AnyJsCommand, FocusableJsCommand, IntoAnyJsCommand};
use crate::core::js_command::helpers::build_key_event_object;
use crate::core::keyboard::{AppKeyEvent, KeyEventType};
use crate::elements::text::span_data::SpanData;
use crate::elements::text::text_layout::TextLayoutData;

fn extract_callable(value: &JsValue) -> Option<JsFunction> {
    value.as_object().and_then(JsFunction::from_object)
}

#[derive(Clone)]
pub(crate) struct LineNavInfo {
    line_start_chars: Vec<usize>,
    line_end_chars: Vec<usize>,
    line_glyph_xs: Vec<Vec<f32>>,
    cursor_xy: (f32, f32),
    current_line: usize,
}

impl LineNavInfo {
    fn extract(ld: &TextLayoutData, cursor_char_idx: usize) -> Self {
        let current_line = ld.line_index_for_char(cursor_char_idx);
        let cursor_xy = ld.cursor_xy_at(cursor_char_idx);

        let mut line_glyph_xs = Vec::new();
        for line_idx in 0..ld.line_infos.len() {
            let start = ld.line_start_char(line_idx);
            let end = ld.line_end_char(line_idx);
            let mut xs = Vec::new();
            for ci in start..end {
                let (x, _) = ld.cursor_xy_at(ci);
                xs.push(x);
            }
            let last_x = ld.runs.last().and_then(|r| r.glyphs.last()).map(|g| g.x + g.advance).unwrap_or(0.0);
            xs.push(last_x);
            line_glyph_xs.push(xs);
        }

        LineNavInfo {
            line_start_chars: (0..ld.line_infos.len())
                .map(|i| ld.line_start_char(i))
                .collect(),
            line_end_chars: (0..ld.line_infos.len())
                .map(|i| ld.line_end_char(i))
                .collect(),
            line_glyph_xs,
            cursor_xy,
            current_line,
        }
    }
}

#[derive(Clone)]
pub(crate) enum EditableTextNotification {
    TextChanged { text: String, enter: bool },
    CursorChanged { position: usize },
    SelectionChanged { anchor: usize, end: usize },
    CompositionStarted,
    CompositionUpdated { text: String },
    CompositionEnded { text: String },
}

impl IntoAnyJsCommand for EditableTextNotification {
    fn into_any_js_command(self) -> AnyJsCommand {
        AnyJsCommand(std::rc::Rc::new(self))
    }
}

pub struct EditableTextElement {
    pub(crate) spans: Vec<SpanData>,
    pub(crate) font_size: f64,
    pub(crate) color: Option<Color>,
    pub(crate) cursor_position: usize,
    pub(crate) cursor_color: Option<Color>,
    pub(crate) placeholder: Option<String>,
    pub(crate) placeholder_color: Option<Color>,
    pub(crate) multiline: bool,
    pub(crate) cached_layout: Option<TextLayoutData>,
    pub(crate) selection_anchor: usize,
    pub(crate) selection_end: usize,
    pub(crate) composition_text: Option<String>,
    pub(crate) composition_start: usize,
    on_key_down: Option<JsFunction>,
    on_key_up: Option<JsFunction>,
    on_focus: Option<JsFunction>,
    on_blur: Option<JsFunction>,
    on_input: Option<JsFunction>,
    on_cursor_change: Option<JsFunction>,
    on_selection_change: Option<JsFunction>,
    on_composition_start: Option<JsFunction>,
    on_composition_update: Option<JsFunction>,
    on_composition_end: Option<JsFunction>,
}

impl Default for EditableTextElement {
    fn default() -> Self {
        Self::new()
    }
}

impl EditableTextElement {
    pub fn new() -> Self {
        EditableTextElement {
            spans: Vec::new(),
            font_size: 14.0,
            color: None,
            cursor_position: 0,
            cursor_color: None,
            placeholder: None,
            placeholder_color: None,
            multiline: false,
            cached_layout: None,
            selection_anchor: 0,
            selection_end: 0,
            composition_text: None,
            composition_start: 0,
            on_key_down: None,
            on_key_up: None,
            on_focus: None,
            on_blur: None,
            on_input: None,
            on_cursor_change: None,
            on_selection_change: None,
            on_composition_start: None,
            on_composition_update: None,
            on_composition_end: None,
        }
    }

    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    pub fn has_selection(&self) -> bool {
        self.selection_anchor != self.selection_end
    }

    fn selection_range(&self) -> (usize, usize) {
        let (a, b) = (self.selection_anchor, self.selection_end);
        if a <= b { (a, b) } else { (b, a) }
    }

    fn clear_selection(&mut self) {
        self.selection_anchor = self.cursor_position;
        self.selection_end = self.cursor_position;
    }

    fn delete_selection(&mut self) {
        if !self.has_selection() { return; }
        let (start, end) = self.selection_range();
        self.delete_range(start, end);
        self.cursor_position = start;
        self.clear_selection();
    }

    fn is_composing(&self) -> bool {
        self.composition_text.is_some()
    }

    fn full_len(&self) -> usize {
        self.spans.iter().map(|s| s.text.len()).sum()
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

    fn delete_range(&mut self, start: usize, end: usize) {
        if start >= end || self.spans.is_empty() {
            return;
        }
        let total = self.full_len();
        let end = end.min(total);
        let start = start.min(total);
        if start >= end {
            return;
        }

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

    fn insert_char_at(&mut self, pos: usize, ch: char) {
        self.insert_at(pos, &ch.to_string());
    }

    fn insert_str_at(&mut self, pos: usize, text: &str) {
        self.insert_at(pos, text);
    }

    pub(crate) fn composition_display_text(&self) -> String {
        let base = self.text();
        if let Some(ref comp) = self.composition_text {
            let start = self.composition_start.min(base.len());
            format!("{}{}{}", &base[..start], comp, &base[start..])
        } else {
            base
        }
    }

    fn handle_key_event(
        &mut self,
        key: &str,
        ctrl: bool,
        meta: bool,
        shift: bool,
        nav_info: Option<&LineNavInfo>,
    ) -> bool {
        let full = self.text();
        match key {
            "Backspace" => {
                if self.has_selection() {
                    self.delete_selection();
                    true
                } else if self.cursor_position > 0 {
                    let new_pos = prev_grapheme_boundary(&full, self.cursor_position);
                    let end = next_grapheme_boundary(&full, new_pos);
                    self.delete_range(new_pos, end);
                    self.cursor_position = new_pos;
                    self.clear_selection();
                    true
                } else {
                    false
                }
            }
            "Delete" => {
                if self.has_selection() {
                    self.delete_selection();
                    true
                } else if self.cursor_position < self.full_len() {
                    let end = next_grapheme_boundary(&full, self.cursor_position);
                    self.delete_range(self.cursor_position, end);
                    self.clear_selection();
                    true
                } else {
                    false
                }
            }
            "ArrowLeft" => {
                if shift {
                    let new_end = prev_grapheme_boundary(&full, self.selection_end);
                    if !self.has_selection() { self.selection_anchor = self.cursor_position; }
                    self.selection_end = new_end;
                    self.cursor_position = new_end;
                    true
                } else if self.has_selection() {
                    let (start, _) = self.selection_range();
                    self.cursor_position = start;
                    self.clear_selection();
                    true
                } else {
                    self.cursor_position = prev_grapheme_boundary(&full, self.cursor_position);
                    self.clear_selection();
                    true
                }
            }
            "ArrowRight" => {
                if shift {
                    let new_end = next_grapheme_boundary(&full, self.selection_end);
                    if !self.has_selection() { self.selection_anchor = self.cursor_position; }
                    self.selection_end = new_end;
                    self.cursor_position = new_end;
                    true
                } else if self.has_selection() {
                    let (_, end) = self.selection_range();
                    self.cursor_position = end;
                    self.clear_selection();
                    true
                } else {
                    self.cursor_position = next_grapheme_boundary(&full, self.cursor_position);
                    self.clear_selection();
                    true
                }
            }
            "ArrowUp" if self.multiline => {
                if let Some(info) = nav_info {
                    self.move_vertical(info, -1, shift)
                } else {
                    false
                }
            }
            "ArrowDown" if self.multiline => {
                if let Some(info) = nav_info {
                    self.move_vertical(info, 1, shift)
                } else {
                    false
                }
            }
            "Home" => {
                if self.multiline {
                    if let Some(info) = nav_info {
                        let line_start = info.line_start_chars[info.current_line];
                        let target = char_to_byte_offset(&full, line_start);
                        if shift {
                            if !self.has_selection() { self.selection_anchor = self.cursor_position; }
                            self.selection_end = target;
                            self.cursor_position = target;
                        } else {
                            self.cursor_position = target;
                            self.clear_selection();
                        }
                        true
                    } else {
                        false
                    }
                } else if shift {
                    if !self.has_selection() { self.selection_anchor = self.cursor_position; }
                    self.selection_end = 0;
                    self.cursor_position = 0;
                    true
                } else {
                    self.cursor_position = 0;
                    self.clear_selection();
                    true
                }
            }
            "End" => {
                let len = self.full_len();
                if self.multiline {
                    if let Some(info) = nav_info {
                        let line_end = info.line_end_chars[info.current_line];
                        let target = char_to_byte_offset(&full, line_end);
                        if shift {
                            if !self.has_selection() { self.selection_anchor = self.cursor_position; }
                            self.selection_end = target;
                            self.cursor_position = target;
                        } else {
                            self.cursor_position = target;
                            self.clear_selection();
                        }
                        true
                    } else {
                        false
                    }
                } else if shift {
                    if !self.has_selection() { self.selection_anchor = self.cursor_position; }
                    self.selection_end = len;
                    self.cursor_position = len;
                    true
                } else {
                    self.cursor_position = len;
                    self.clear_selection();
                    true
                }
            }
            "a" if ctrl || meta => {
                let len = self.full_len();
                self.selection_anchor = 0;
                self.selection_end = len;
                self.cursor_position = len;
                true
            }
            "Enter" => {
                if self.multiline {
                    if self.has_selection() { self.delete_selection(); }
                    self.insert_char_at(self.cursor_position, '\n');
                    self.cursor_position += '\n'.len_utf8();
                    self.clear_selection();
                    true
                } else {
                    true
                }
            }
            _ => {
                if key.len() == 1 && !ctrl && !meta && !self.is_composing() {
                    let ch = key.chars().next().unwrap();
                    if self.has_selection() { self.delete_selection(); }
                    self.insert_char_at(self.cursor_position, ch);
                    self.cursor_position += ch.len_utf8();
                    self.clear_selection();
                    true
                } else {
                    false
                }
            }
        }
    }

    fn move_vertical(&mut self, info: &LineNavInfo, direction: i32, shift: bool) -> bool {
        let full = self.text();
        let current_line = info.current_line;
        let cursor_x = info.cursor_xy.0;
        let num_lines = info.line_start_chars.len();

        let target_line = if direction < 0 {
            current_line.saturating_sub(1)
        } else {
            (current_line + 1).min(num_lines - 1)
        };

        if target_line == current_line {
            return false;
        }

        let target_char = {
            let xs = &info.line_glyph_xs[target_line];
            let line_start = info.line_start_chars[target_line];
            let mut best_idx = xs.len().saturating_sub(1);
            let mut best_dist = f32::MAX;
            for (i, &x) in xs.iter().enumerate() {
                let dist = (x - cursor_x).abs();
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = i;
                }
            }
            line_start + best_idx
        };

        let target_byte = char_to_byte_offset(&full, target_char);

        if shift {
            if !self.has_selection() { self.selection_anchor = self.cursor_position; }
            self.selection_end = target_byte;
            self.cursor_position = target_byte;
        } else {
            self.cursor_position = target_byte;
            self.clear_selection();
        }
        true
    }

    fn char_index_at(&self, local_position: &Offset) -> usize {
        self.cached_layout
            .as_ref()
            .map(|ld| {
                if self.multiline {
                    ld.char_index_at_xy(local_position.x as f32, local_position.y as f32)
                } else {
                    ld.char_index_at_x(local_position.x as f32)
                }
            })
            .unwrap_or(0)
    }
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

fn char_to_byte_offset(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map(|(i, _)| i).unwrap_or(s.len())
}

fn byte_to_char_offset(s: &str, byte_pos: usize) -> usize {
    s[..byte_pos.min(s.len())].chars().count()
}

impl ElementTrace for EditableTextElement {
    fn trace_label(&self) -> String {
        let text = self.text();
        if text.is_empty() {
            String::new()
        } else {
            let t = if text.len() > 20 { &text[..20] } else { &text };
            format!("\"{}\"", t)
        }
    }
}

impl ElementOnUpdate for EditableTextElement {
    fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue) {
        match key.to_std_string_escaped().as_str() {
            "spans" => {
                self.spans = crate::elements::text::span_data::extract_spans_from_js(value, ctx);
                self.cursor_position = self.full_len();
                self.clear_selection();
                self.composition_text = None;
            }
            "fontSize" => {
                self.font_size = value.as_number().unwrap_or(14.0);
            }
            "color" => {
                self.color = extract_color(value, ctx);
            }
            "cursorColor" => {
                self.cursor_color = extract_color(value, ctx);
            }
            "placeholder" => {
                self.placeholder = value.as_string().map(|s| s.to_std_string_escaped());
            }
            "placeholderColor" => {
                self.placeholder_color = extract_color(value, ctx);
            }
            "multiline" => {
                self.multiline = value.as_boolean().unwrap_or(value.to_boolean());
            }
            "onKeyDown" => { self.on_key_down = extract_callable(value); }
            "onKeyUp" => { self.on_key_up = extract_callable(value); }
            "onFocus" => { self.on_focus = extract_callable(value); }
            "onBlur" => { self.on_blur = extract_callable(value); }
            "onInput" => { self.on_input = extract_callable(value); }
            "onCursorChange" => { self.on_cursor_change = extract_callable(value); }
            "onSelectionChange" => { self.on_selection_change = extract_callable(value); }
            "onCompositionStart" => { self.on_composition_start = extract_callable(value); }
            "onCompositionUpdate" => { self.on_composition_update = extract_callable(value); }
            "onCompositionEnd" => { self.on_composition_end = extract_callable(value); }
            _ => {}
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        match key.to_std_string_escaped().as_str() {
            "spans" => { self.spans.clear(); self.cursor_position = 0; self.clear_selection(); }
            "fontSize" => self.font_size = 14.0,
            "color" => self.color = None,
            "cursorColor" => self.cursor_color = None,
            "placeholder" => self.placeholder = None,
            "placeholderColor" => self.placeholder_color = None,
            "multiline" => self.multiline = false,
            "onKeyDown" => self.on_key_down = None,
            "onKeyUp" => self.on_key_up = None,
            "onFocus" => self.on_focus = None,
            "onBlur" => self.on_blur = None,
            "onInput" => self.on_input = None,
            "onCursorChange" => self.on_cursor_change = None,
            "onSelectionChange" => self.on_selection_change = None,
            "onCompositionStart" => self.on_composition_start = None,
            "onCompositionUpdate" => self.on_composition_update = None,
            "onCompositionEnd" => self.on_composition_end = None,
            _ => {}
        }
    }
}

impl ElementOnFocus for EditableTextElement {}

impl ElementOnGesture for EditableTextElement {
    fn on_gesture_event(
        &mut self,
        cx: &mut ElementOnGestureContext,
        event: &ComposedGestureEvent,
    ) {
        let full = self.text();
        match event {
            ComposedGestureEvent::PointerDown { local_position } => {
                cx.request_own_focus();
                let char_idx = self.char_index_at(local_position);
                let byte_pos = char_to_byte_offset(&full, char_idx);
                self.cursor_position = byte_pos;
                self.selection_anchor = byte_pos;
                self.selection_end = byte_pos;
                cx.request_redraw();
            }
            ComposedGestureEvent::PointerMove { local_position } => {
                let char_idx = self.char_index_at(local_position);
                let byte_pos = char_to_byte_offset(&full, char_idx);
                if byte_pos != self.selection_end {
                    self.selection_end = byte_pos;
                    self.cursor_position = byte_pos;
                    cx.request_redraw();
                }
            }
        }
    }
}

impl ElementOnKeyboard for EditableTextElement {
    fn on_keyboard_event(
        &mut self,
        cx: &mut ElementOnKeyboardContext,
        event: &AppKeyEvent,
    ) {
        if event.event_type != KeyEventType::Down {
            return;
        }

        let prev_text = self.text();
        let prev_cursor = self.cursor_position;
        let prev_anchor = self.selection_anchor;
        let prev_end = self.selection_end;

        let nav_info = self.cached_layout.as_ref().map(|ld| {
            let cursor_char = byte_to_char_offset(&prev_text, self.cursor_position);
            LineNavInfo::extract(ld, cursor_char)
        });

        let changed = self.handle_key_event(
            &event.key,
            event.modifiers.ctrl,
            event.modifiers.meta,
            event.modifiers.shift,
            nav_info.as_ref(),
        );

        if changed {
            cx.request_redraw();

            let new_text = self.text();
            if new_text != prev_text {
                let enter = event.key == "Enter" && !self.multiline;
                cx.push_js_command(EditableTextNotification::TextChanged {
                    text: new_text,
                    enter,
                });
            }
            if self.cursor_position != prev_cursor {
                cx.push_js_command(EditableTextNotification::CursorChanged {
                    position: self.cursor_position,
                });
            }
            if self.selection_anchor != prev_anchor || self.selection_end != prev_end {
                cx.push_js_command(EditableTextNotification::SelectionChanged {
                    anchor: self.selection_anchor,
                    end: self.selection_end,
                });
            }
        }
    }
}

impl ElementOnIme for EditableTextElement {
    fn on_ime_event(
        &mut self,
        cx: &mut ElementOnImeContext,
        event: &AppImeEvent,
    ) {
        match event {
            AppImeEvent::CompositionStart => {
                self.composition_text = Some(String::new());
                self.composition_start = self.cursor_position;
                cx.push_js_command(EditableTextNotification::CompositionStarted);
                cx.request_redraw();
            }
            AppImeEvent::CompositionUpdate { text, .. } => {
                if self.composition_text.is_some() {
                    self.composition_text = Some(text.clone());
                    cx.push_js_command(EditableTextNotification::CompositionUpdated {
                        text: text.clone(),
                    });
                    cx.request_redraw();
                }
            }
            AppImeEvent::CompositionEnd { text } => {
                if self.composition_text.take().is_some() {
                    let start = self.composition_start.min(self.full_len());
                    self.insert_str_at(start, text);
                    self.cursor_position = start + text.len();
                    self.clear_selection();

                    let new_text = self.text();
                    cx.push_js_command(EditableTextNotification::CompositionEnded {
                        text: text.clone(),
                    });
                    cx.push_js_command(EditableTextNotification::TextChanged {
                        text: new_text,
                        enter: false,
                    });
                    cx.push_js_command(EditableTextNotification::CursorChanged {
                        position: self.cursor_position,
                    });
                    cx.request_redraw();
                }
            }
        }
    }
}

impl ElementJsCallbackEmitter for EditableTextElement {
    fn emit_js_callback(
        &self,
        context: &mut Context,
        command: AnyJsCommand,
    ) -> Option<(JsFunction, Vec<JsValue>)> {
        use boa_engine::js_string;

        if let Some(c) = command.downcast_ref::<EditableTextNotification>() {
            match c {
                EditableTextNotification::TextChanged { text, enter } => {
                    self.on_input.as_ref().map(|h| {
                        (h.clone(), vec![JsValue::from(js_string!(text.as_str())), JsValue::from(*enter)])
                    })
                }
                EditableTextNotification::CursorChanged { position } => {
                    self.on_cursor_change.as_ref().map(|h| {
                        (h.clone(), vec![JsValue::from(*position as f64)])
                    })
                }
                EditableTextNotification::SelectionChanged { anchor, end } => {
                    self.on_selection_change.as_ref().map(|h| {
                        (h.clone(), vec![JsValue::from(*anchor as f64), JsValue::from(*end as f64)])
                    })
                }
                EditableTextNotification::CompositionStarted => {
                    self.on_composition_start.as_ref().map(|h| (h.clone(), vec![]))
                }
                EditableTextNotification::CompositionUpdated { text } => {
                    self.on_composition_update.as_ref().map(|h| {
                        (h.clone(), vec![JsValue::from(js_string!(text.as_str()))])
                    })
                }
                EditableTextNotification::CompositionEnded { text } => {
                    self.on_composition_end.as_ref().map(|h| {
                        (h.clone(), vec![JsValue::from(js_string!(text.as_str()))])
                    })
                }
            }
        } else if let Some(c) = command.downcast_ref::<FocusableJsCommand>() {
            match c {
                FocusableJsCommand::KeyDown { key, code, modifiers } => {
                    self.on_key_down.as_ref().map(|h| {
                        let event_obj = build_key_event_object(key, code, modifiers, context);
                        (h.clone(), vec![event_obj])
                    })
                }
                FocusableJsCommand::KeyUp { key, code, modifiers } => {
                    self.on_key_up.as_ref().map(|h| {
                        let event_obj = build_key_event_object(key, code, modifiers, context);
                        (h.clone(), vec![event_obj])
                    })
                }
                FocusableJsCommand::Focus => {
                    self.on_focus.as_ref().map(|h| (h.clone(), vec![]))
                }
                FocusableJsCommand::Blur => {
                    self.on_blur.as_ref().map(|h| (h.clone(), vec![]))
                }
            }
        } else {
            None
        }
    }
}
