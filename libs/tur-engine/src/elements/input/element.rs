use boa_engine::{Context, JsString, JsValue};
use tur_shared::Color;

use crate::core::elements::ElementOnUpdate;
use crate::core::elements::ElementTrace;
use crate::elements::text::text_layout::TextLayoutData;

pub struct InputElement {
    pub(crate) content: String,
    pub(crate) font_size: f64,
    pub(crate) color: Option<Color>,
    pub(crate) cursor_position: usize,
    pub(crate) cursor_color: Option<Color>,
    pub(crate) focused: bool,
    pub(crate) placeholder: Option<String>,
    pub(crate) placeholder_color: Option<Color>,
    pub(crate) cached_layout: Option<TextLayoutData>,
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
            cached_layout: None,
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

    pub fn set_text(&mut self, text: &str) {
        self.content = text.to_string();
        self.cursor_position = self.content.len();
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if focused {
            self.cursor_position = self.content.len();
        }
    }

    pub fn handle_key_event(&mut self, key: &str, ctrl: bool, meta: bool) -> InputEditResult {
        let text = &mut self.content;
        let pos = &mut self.cursor_position;

        match key {
            "Backspace" => {
                if *pos > 0 {
                    *pos = prev_char_boundary(text, *pos);
                    let end = next_char_boundary(text, *pos);
                    text.replace_range(*pos..end, "");
                    InputEditResult::TextChanged(text.clone())
                } else {
                    InputEditResult::Handled
                }
            }
            "Delete" => {
                if *pos < text.len() {
                    let end = next_char_boundary(text, *pos);
                    text.replace_range(*pos..end, "");
                    InputEditResult::TextChanged(text.clone())
                } else {
                    InputEditResult::Handled
                }
            }
            "ArrowLeft" => {
                *pos = prev_char_boundary(text, *pos);
                InputEditResult::CursorMoved
            }
            "ArrowRight" => {
                *pos = next_char_boundary(text, *pos);
                InputEditResult::CursorMoved
            }
            "Home" => {
                *pos = 0;
                InputEditResult::CursorMoved
            }
            "End" => {
                *pos = text.len();
                InputEditResult::CursorMoved
            }
            "Enter" => InputEditResult::EnterPressed(text.clone()),
            _ => {
                if key.len() == 1 && !ctrl && !meta {
                    let ch = key.chars().next().unwrap();
                    text.insert(*pos, ch);
                    *pos += ch.len_utf8();
                    InputEditResult::TextChanged(text.clone())
                } else {
                    InputEditResult::NotHandled
                }
            }
        }
    }
}

pub enum InputEditResult {
    NotHandled,
    Handled,
    CursorMoved,
    TextChanged(String),
    EnterPressed(String),
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
        }
    }
}
