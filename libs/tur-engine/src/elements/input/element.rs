use boa_engine::{Context, JsString, JsValue};
use tur_shared::Color;

use crate::core::elements::ElementOnKeyboard;
use crate::core::elements::ElementOnUpdate;
use crate::core::elements::ElementTrace;
use crate::core::elements::KeyboardResult;
use crate::core::keyboard::{AppKeyEvent, KeyEventType};
use crate::elements::text::text_layout::TextLayoutData;

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

pub(crate) struct InputChanges {
    pub text_changed: bool,
    pub cursor_changed: bool,
    pub selection_changed: bool,
    pub enter: bool,
}

pub struct InputElement {
    pub(crate) content: String,
    pub(crate) font_size: f64,
    pub(crate) color: Option<Color>,
    pub(crate) cursor_position: usize,
    pub(crate) cursor_color: Option<Color>,
    pub(crate) focused: bool,
    pub(crate) placeholder: Option<String>,
    pub(crate) placeholder_color: Option<Color>,
    pub(crate) multiline: bool,
    pub(crate) cached_layout: Option<TextLayoutData>,
    pub(crate) selection_anchor: usize,
    pub(crate) selection_end: usize,
    pub(crate) text_changed: bool,
    pub(crate) cursor_changed: bool,
    pub(crate) selection_changed: bool,
    pub(crate) enter_flag: bool,
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
            focused: false,
            placeholder: None,
            placeholder_color: None,
            multiline: false,
            cached_layout: None,
            selection_anchor: 0,
            selection_end: 0,
            text_changed: false,
            cursor_changed: false,
            selection_changed: false,
            enter_flag: false,
        }
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn is_focused(&self) -> bool {
        self.focused
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

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if focused {
            self.cursor_position = self.content.len();
            self.clear_selection();
        }
    }

    pub(crate) fn drain_changes(&mut self) -> InputChanges {
        let changes = InputChanges {
            text_changed: self.text_changed,
            cursor_changed: self.cursor_changed,
            selection_changed: self.selection_changed,
            enter: self.enter_flag,
        };
        self.text_changed = false;
        self.cursor_changed = false;
        self.selection_changed = false;
        self.enter_flag = false;
        changes
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
                    self.text_changed = true;
                    KeyboardResult::NeedsDraw
                } else if self.cursor_position > 0 {
                    self.cursor_position = prev_char_boundary(&self.content, self.cursor_position);
                    let end = next_char_boundary(&self.content, self.cursor_position);
                    self.content.replace_range(self.cursor_position..end, "");
                    self.clear_selection();
                    self.text_changed = true;
                    KeyboardResult::NeedsDraw
                } else {
                    KeyboardResult::Handled
                }
            }
            "Delete" => {
                if self.has_selection() {
                    self.delete_selection();
                    self.text_changed = true;
                    KeyboardResult::NeedsDraw
                } else if self.cursor_position < self.content.len() {
                    let end = next_char_boundary(&self.content, self.cursor_position);
                    self.content.replace_range(self.cursor_position..end, "");
                    self.clear_selection();
                    self.text_changed = true;
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
                    self.cursor_changed = true;
                    self.selection_changed = true;
                    KeyboardResult::NeedsDraw
                } else if self.has_selection() {
                    let (start, _) = self.selection_range();
                    self.cursor_position = start;
                    self.clear_selection();
                    self.cursor_changed = true;
                    KeyboardResult::NeedsDraw
                } else {
                    self.cursor_position = prev_char_boundary(&self.content, self.cursor_position);
                    self.clear_selection();
                    self.cursor_changed = true;
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
                    self.cursor_changed = true;
                    self.selection_changed = true;
                    KeyboardResult::NeedsDraw
                } else if self.has_selection() {
                    let (_, end) = self.selection_range();
                    self.cursor_position = end;
                    self.clear_selection();
                    self.cursor_changed = true;
                    KeyboardResult::NeedsDraw
                } else {
                    self.cursor_position = next_char_boundary(&self.content, self.cursor_position);
                    self.clear_selection();
                    self.cursor_changed = true;
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
                            self.cursor_changed = true;
                            self.selection_changed = true;
                            KeyboardResult::NeedsDraw
                        } else {
                            self.cursor_position = target;
                            self.clear_selection();
                            self.cursor_changed = true;
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
                    self.cursor_changed = true;
                    self.selection_changed = true;
                    KeyboardResult::NeedsDraw
                } else {
                    self.cursor_position = 0;
                    self.clear_selection();
                    self.cursor_changed = true;
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
                            self.cursor_changed = true;
                            self.selection_changed = true;
                            KeyboardResult::NeedsDraw
                        } else {
                            self.cursor_position = target;
                            self.clear_selection();
                            self.cursor_changed = true;
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
                    self.cursor_changed = true;
                    self.selection_changed = true;
                    KeyboardResult::NeedsDraw
                } else {
                    self.cursor_position = self.content.len();
                    self.clear_selection();
                    self.cursor_changed = true;
                    KeyboardResult::NeedsDraw
                }
            }
            "a" if ctrl || meta => {
                self.select_all();
                self.cursor_changed = true;
                self.selection_changed = true;
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
                    self.text_changed = true;
                    self.enter_flag = true;
                    KeyboardResult::NeedsDraw
                } else {
                    self.text_changed = true;
                    self.enter_flag = true;
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
                    self.text_changed = true;
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
            self.cursor_changed = true;
            self.selection_changed = true;
            KeyboardResult::NeedsDraw
        } else {
            self.cursor_position = target_byte;
            self.clear_selection();
            self.cursor_changed = true;
            KeyboardResult::NeedsDraw
        }
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

impl ElementOnKeyboard for InputElement {
    fn on_keyboard_event(&mut self, event: &AppKeyEvent) -> KeyboardResult {
        if event.event_type != KeyEventType::Down {
            return KeyboardResult::NotHandled;
        }
        let nav_info = self.cached_layout.as_ref().map(|ld| {
            let cursor_char = byte_to_char_offset(&self.content, self.cursor_position);
            LineNavInfo::extract(ld, cursor_char)
        });
        self.handle_key_event(
            &event.key,
            event.modifiers.ctrl,
            event.modifiers.meta,
            event.modifiers.shift,
            nav_info.as_ref(),
        )
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
        }
    }
}
