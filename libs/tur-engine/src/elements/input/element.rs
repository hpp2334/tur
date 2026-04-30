use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsString, JsValue};
use tur_shared::{Color, Offset};

use crate::core::elements::{
    ComposedGestureEvent, ElementJsEventEmitter, ElementOnFocus, ElementOnGesture,
    ElementOnGestureContext, ElementOnKeyboard, ElementOnKeyboardContext, ElementOnUpdate,
    ElementTrace, GestureResult, KeyboardResult,
};
use crate::core::js_event::{AnyJsEvent, FocusableJsEvent, InputJsEvent};
use crate::core::js_event::helpers::build_key_event_object;
use crate::core::keyboard::{AppKeyEvent, KeyEventType};
use crate::elements::text::text_layout::TextLayoutData;

fn extract_callable(value: &JsValue) -> Option<JsObject> {
    value.as_object().and_then(|o| {
        if o.is_callable() {
            Some(o.clone())
        } else {
            None
        }
    })
}

#[derive(Clone)]
pub(crate) struct LineNavInfo {
    pub line_baselines: Vec<f32>,
    pub line_start_chars: Vec<usize>,
    pub line_end_chars: Vec<usize>,
    pub line_glyph_xs: Vec<Vec<f32>>,
    pub cursor_xy: (f32, f32),
    pub current_line: usize,
}

impl LineNavInfo {
    pub fn extract(ld: &TextLayoutData, cursor_char_idx: usize) -> Self {
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
            line_baselines: ld.line_infos.iter().map(|l| l.baseline).collect(),
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

pub struct InputElement {
    pub(crate) content: String,
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
    on_key_down: Option<JsObject>,
    on_key_up: Option<JsObject>,
    on_focus: Option<JsObject>,
    on_blur: Option<JsObject>,
    on_input: Option<JsObject>,
    on_cursor_change: Option<JsObject>,
    on_selection_change: Option<JsObject>,
}

impl Default for InputElement {
    fn default() -> Self {
        Self::new()
    }
}

impl InputElement {
    pub fn new() -> Self {
        InputElement {
            content: String::new(),
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
            on_key_down: None,
            on_key_up: None,
            on_focus: None,
            on_blur: None,
            on_input: None,
            on_cursor_change: None,
            on_selection_change: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    pub fn has_selection(&self) -> bool {
        self.selection_anchor != self.selection_end
    }

    pub fn selection_anchor(&self) -> usize {
        self.selection_anchor
    }

    pub fn selection_end(&self) -> usize {
        self.selection_end
    }

    pub fn selection_range(&self) -> (usize, usize) {
        let (a, b) = (self.selection_anchor, self.selection_end);
        if a <= b {
            (a, b)
        } else {
            (b, a)
        }
    }

    pub fn selected_text(&self) -> &str {
        if !self.has_selection() {
            return "";
        }
        let (start, end) = self.selection_range();
        &self.content[start..end]
    }

    pub fn select_all(&mut self) {
        let len = self.content.len();
        self.selection_anchor = 0;
        self.selection_end = len;
        self.cursor_position = len;
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = self.cursor_position;
        self.selection_end = self.cursor_position;
    }

    pub fn set_selection(&mut self, anchor: usize, end: usize) {
        self.selection_anchor = anchor.min(self.content.len());
        self.selection_end = end.min(self.content.len());
        self.cursor_position = self.selection_end;
    }

    pub fn set_text(&mut self, text: &str) {
        self.content = text.to_string();
        self.cursor_position = self.content.len();
        self.clear_selection();
    }

    fn delete_selection(&mut self) {
        if !self.has_selection() {
            return;
        }
        let (start, end) = self.selection_range();
        self.content.replace_range(start..end, "");
        self.cursor_position = start;
        self.clear_selection();
    }

    pub(crate) fn handle_key_event(
        &mut self,
        key: &str,
        ctrl: bool,
        meta: bool,
        shift: bool,
        nav_info: Option<&LineNavInfo>,
    ) -> KeyboardResult {
        match key {
            "Backspace" => {
                if self.has_selection() {
                    self.delete_selection();
                    KeyboardResult::NeedsDraw
                } else if self.cursor_position > 0 {
                    self.cursor_position = prev_char_boundary(&self.content, self.cursor_position);
                    let end = next_char_boundary(&self.content, self.cursor_position);
                    self.content.replace_range(self.cursor_position..end, "");
                    self.clear_selection();
                    KeyboardResult::NeedsDraw
                } else {
                    KeyboardResult::Handled
                }
            }
            "Delete" => {
                if self.has_selection() {
                    self.delete_selection();
                    KeyboardResult::NeedsDraw
                } else if self.cursor_position < self.content.len() {
                    let end = next_char_boundary(&self.content, self.cursor_position);
                    self.content.replace_range(self.cursor_position..end, "");
                    self.clear_selection();
                    KeyboardResult::NeedsDraw
                } else {
                    KeyboardResult::Handled
                }
            }
            "ArrowLeft" => {
                if shift {
                    let new_end = prev_char_boundary(&self.content, self.selection_end);
                    if !self.has_selection() {
                        self.selection_anchor = self.cursor_position;
                    }
                    self.selection_end = new_end;
                    self.cursor_position = new_end;


                    KeyboardResult::NeedsDraw
                } else if self.has_selection() {
                    let (start, _) = self.selection_range();
                    self.cursor_position = start;
                    self.clear_selection();

                    KeyboardResult::NeedsDraw
                } else {
                    self.cursor_position = prev_char_boundary(&self.content, self.cursor_position);
                    self.clear_selection();

                    KeyboardResult::NeedsDraw
                }
            }
            "ArrowRight" => {
                if shift {
                    let new_end = next_char_boundary(&self.content, self.selection_end);
                    if !self.has_selection() {
                        self.selection_anchor = self.cursor_position;
                    }
                    self.selection_end = new_end;
                    self.cursor_position = new_end;


                    KeyboardResult::NeedsDraw
                } else if self.has_selection() {
                    let (_, end) = self.selection_range();
                    self.cursor_position = end;
                    self.clear_selection();

                    KeyboardResult::NeedsDraw
                } else {
                    self.cursor_position = next_char_boundary(&self.content, self.cursor_position);
                    self.clear_selection();

                    KeyboardResult::NeedsDraw
                }
            }
            "ArrowUp" if self.multiline => {
                if let Some(info) = nav_info {
                    self.move_vertical(info, -1, shift)
                } else {
                    KeyboardResult::NotHandled
                }
            }
            "ArrowDown" if self.multiline => {
                if let Some(info) = nav_info {
                    self.move_vertical(info, 1, shift)
                } else {
                    KeyboardResult::NotHandled
                }
            }
            "Home" => {
                if self.multiline {
                    if let Some(info) = nav_info {
                        let line_idx = info.current_line;
                        let line_start = info.line_start_chars[line_idx];
                        let target = char_to_byte_offset(&self.content, line_start);
                        if shift {
                            if !self.has_selection() {
                                self.selection_anchor = self.cursor_position;
                            }
                            self.selection_end = target;
                            self.cursor_position = target;
        
        
                            KeyboardResult::NeedsDraw
                        } else {
                            self.cursor_position = target;
                            self.clear_selection();
        
                            KeyboardResult::NeedsDraw
                        }
                    } else {
                        KeyboardResult::NotHandled
                    }
                } else if shift {
                    if !self.has_selection() {
                        self.selection_anchor = self.cursor_position;
                    }
                    self.selection_end = 0;
                    self.cursor_position = 0;


                    KeyboardResult::NeedsDraw
                } else {
                    self.cursor_position = 0;
                    self.clear_selection();

                    KeyboardResult::NeedsDraw
                }
            }
            "End" => {
                if self.multiline {
                    if let Some(info) = nav_info {
                        let line_idx = info.current_line;
                        let line_end = info.line_end_chars[line_idx];
                        let target = char_to_byte_offset(&self.content, line_end);
                        if shift {
                            if !self.has_selection() {
                                self.selection_anchor = self.cursor_position;
                            }
                            self.selection_end = target;
                            self.cursor_position = target;
        
        
                            KeyboardResult::NeedsDraw
                        } else {
                            self.cursor_position = target;
                            self.clear_selection();
        
                            KeyboardResult::NeedsDraw
                        }
                    } else {
                        KeyboardResult::NotHandled
                    }
                } else if shift {
                    if !self.has_selection() {
                        self.selection_anchor = self.cursor_position;
                    }
                    self.selection_end = self.content.len();
                    self.cursor_position = self.content.len();


                    KeyboardResult::NeedsDraw
                } else {
                    self.cursor_position = self.content.len();
                    self.clear_selection();

                    KeyboardResult::NeedsDraw
                }
            }
            "a" if ctrl || meta => {
                self.select_all();
                KeyboardResult::NeedsDraw
            }
            "Enter" => {
                if self.multiline {
                    if self.has_selection() {
                        self.delete_selection();
                    }
                    self.content.insert(self.cursor_position, '\n');
                    self.cursor_position += '\n'.len_utf8();
                    self.clear_selection();


                    KeyboardResult::NeedsDraw
                } else {


                    KeyboardResult::NeedsDraw
                }
            }
            _ => {
                if key.len() == 1 && !ctrl && !meta {
                    let ch = key.chars().next().unwrap();
                    if self.has_selection() {
                        self.delete_selection();
                    }
                    self.content.insert(self.cursor_position, ch);
                    self.cursor_position += ch.len_utf8();
                    self.clear_selection();

                    KeyboardResult::NeedsDraw
                } else {
                    KeyboardResult::NotHandled
                }
            }
        }
    }

    fn move_vertical(
        &mut self,
        info: &LineNavInfo,
        direction: i32,
        shift: bool,
    ) -> KeyboardResult {
        let current_line = info.current_line;
        let cursor_x = info.cursor_xy.0;

        let target_line = if direction < 0 {
            current_line.saturating_sub(1)
        } else {
            (current_line + 1).min(info.line_baselines.len() - 1)
        };

        if target_line == current_line {
            return KeyboardResult::Handled;
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

        let target_byte = char_to_byte_offset(&self.content, target_char);

        if shift {
            if !self.has_selection() {
                self.selection_anchor = self.cursor_position;
            }
            self.selection_end = target_byte;
            self.cursor_position = target_byte;
            KeyboardResult::NeedsDraw
        } else {
            self.cursor_position = target_byte;
            self.clear_selection();
            KeyboardResult::NeedsDraw
        }
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

fn prev_char_boundary(s: &str, pos: usize) -> usize {
    let mut p = pos;
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    while p > 0 {
        p -= 1;
        if s.is_char_boundary(p) {
            return p;
        }
    }
    0
}

fn next_char_boundary(s: &str, pos: usize) -> usize {
    let mut p = pos;
    while p < s.len() && !s.is_char_boundary(p) {
        p += 1;
    }
    while p < s.len() {
        p += 1;
        if s.is_char_boundary(p) {
            return p;
        }
    }
    s.len()
}

fn char_to_byte_offset(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

fn byte_to_char_offset(s: &str, byte_pos: usize) -> usize {
    s[..byte_pos].chars().count()
}

impl ElementOnGesture for InputElement {
    fn on_gesture_event(
        &mut self,
        event: &ComposedGestureEvent,
        cx: &mut ElementOnGestureContext,
    ) -> GestureResult {
        match event {
            ComposedGestureEvent::PointerDown { local_position } => {
                cx.request_own_focus();
                let char_idx = self.char_index_at(local_position);
                let byte_pos = char_to_byte_offset(&self.content, char_idx);
                self.cursor_position = byte_pos;
                self.selection_anchor = byte_pos;
                self.selection_end = byte_pos;
                cx.request_redraw();
                GestureResult::Handled
            }
            ComposedGestureEvent::PointerMove { local_position } => {
                let char_idx = self.char_index_at(local_position);
                let byte_pos = char_to_byte_offset(&self.content, char_idx);
                self.selection_end = byte_pos;
                self.cursor_position = byte_pos;
                cx.request_redraw();
                GestureResult::Handled
            }
        }
    }
}

impl ElementOnKeyboard for InputElement {
    fn on_keyboard_event(
        &mut self,
        cx: &mut ElementOnKeyboardContext,
        event: &AppKeyEvent,
    ) -> KeyboardResult {
        if event.event_type != KeyEventType::Down {
            return KeyboardResult::NotHandled;
        }

        let prev_content = self.content.clone();
        let prev_cursor = self.cursor_position;
        let prev_anchor = self.selection_anchor;
        let prev_end = self.selection_end;

        let nav_info = self.cached_layout.as_ref().map(|ld| {
            let cursor_char = byte_to_char_offset(&self.content, self.cursor_position);
            LineNavInfo::extract(ld, cursor_char)
        });
        let result = self.handle_key_event(
            &event.key,
            event.modifiers.ctrl,
            event.modifiers.meta,
            event.modifiers.shift,
            nav_info.as_ref(),
        );

        if matches!(result, KeyboardResult::NeedsDraw) {
            cx.request_redraw();

            if self.content != prev_content {
                let enter = event.key == "Enter" && !self.multiline;
                cx.push_js_event(InputJsEvent::Input {
                    text: self.content.clone(),
                    enter,
                });
            }
            if self.cursor_position != prev_cursor {
                cx.push_js_event(InputJsEvent::CursorChange {
                    position: self.cursor_position,
                });
            }
            if self.selection_anchor != prev_anchor || self.selection_end != prev_end {
                cx.push_js_event(InputJsEvent::SelectionChange {
                    anchor: self.selection_anchor,
                    end: self.selection_end,
                });
            }
        }

        result
    }
}

impl ElementTrace for InputElement {
    fn trace_label(&self) -> String {
        format!("\"{}\"", self.content)
    }
}

impl ElementOnUpdate for InputElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "fontSize" {
            self.font_size = value.as_number().unwrap_or(14.0);
        } else if *key == "color" {
            if let Some(s) = value.as_string() {
                self.color = s.to_std_string_escaped().parse().ok();
            }
        } else if *key == "cursorColor" {
            if let Some(s) = value.as_string() {
                self.cursor_color = s.to_std_string_escaped().parse().ok();
            }
        } else if *key == "placeholder" {
            if let Some(s) = value.as_string() {
                self.placeholder = Some(s.to_std_string_escaped());
            } else if value.is_null() || value.is_undefined() {
                self.placeholder = None;
            }
        } else if *key == "placeholderColor" {
            if let Some(s) = value.as_string() {
                self.placeholder_color = s.to_std_string_escaped().parse().ok();
            }
        } else if *key == "multiline" {
            self.multiline = value.as_boolean().unwrap_or(value.to_boolean());
        } else if *key == "onKeyDown" {
            self.on_key_down = extract_callable(value);
        } else if *key == "onKeyUp" {
            self.on_key_up = extract_callable(value);
        } else if *key == "onFocus" {
            self.on_focus = extract_callable(value);
        } else if *key == "onBlur" {
            self.on_blur = extract_callable(value);
        } else if *key == "onInput" {
            self.on_input = extract_callable(value);
        } else if *key == "onCursorChange" {
            self.on_cursor_change = extract_callable(value);
        } else if *key == "onSelectionChange" {
            self.on_selection_change = extract_callable(value);
        }
    }
}

impl ElementOnFocus for InputElement {}

impl ElementJsEventEmitter for InputElement {
    fn flush_js_event(&mut self, event: AnyJsEvent, context: &mut Context) {
        if let Some(e) = event.downcast_ref::<InputJsEvent>() {
            match e {
                InputJsEvent::Input { text, enter } => {
                    if let Some(ref handler) = self.on_input {
                        let text_val = JsValue::from(js_string!(text.as_str()));
                        let enter_val = JsValue::from(*enter);
                        let _ = handler.call(&JsValue::undefined(), &[text_val, enter_val], context);
                    }
                }
                InputJsEvent::CursorChange { position } => {
                    if let Some(ref handler) = self.on_cursor_change {
                        let pos_val = JsValue::from(*position as f64);
                        let _ = handler.call(&JsValue::undefined(), &[pos_val], context);
                    }
                }
                InputJsEvent::SelectionChange { anchor, end } => {
                    if let Some(ref handler) = self.on_selection_change {
                        let start_val = JsValue::from(*anchor as f64);
                        let end_val = JsValue::from(*end as f64);
                        let _ = handler.call(
                            &JsValue::undefined(),
                            &[start_val, end_val],
                            context,
                        );
                    }
                }
            }
        } else if let Some(e) = event.downcast_ref::<FocusableJsEvent>() {
            match e {
                FocusableJsEvent::KeyDown {
                    key,
                    code,
                    modifiers,
                } => {
                    if let Some(ref handler) = self.on_key_down {
                        let event_obj = build_key_event_object(key, code, modifiers, context);
                        let _ = handler.call(&JsValue::undefined(), &[event_obj], context);
                    }
                }
                FocusableJsEvent::KeyUp {
                    key,
                    code,
                    modifiers,
                } => {
                    if let Some(ref handler) = self.on_key_up {
                        let event_obj = build_key_event_object(key, code, modifiers, context);
                        let _ = handler.call(&JsValue::undefined(), &[event_obj], context);
                    }
                }
                FocusableJsEvent::Focus => {
                    if let Some(ref handler) = self.on_focus {
                        let _ = handler.call(&JsValue::undefined(), &[], context);
                    }
                }
                FocusableJsEvent::Blur => {
                    if let Some(ref handler) = self.on_blur {
                        let _ = handler.call(&JsValue::undefined(), &[], context);
                    }
                }
            }
        }
    }
}
