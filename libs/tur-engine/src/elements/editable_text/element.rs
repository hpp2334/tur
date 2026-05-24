use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsString, JsValue};
use tur_shared::{Color, Offset};
use unicode_segmentation::UnicodeSegmentation;

use crate::core::bridge::color::extract_color;
use crate::core::bridge::BoaOpaque;
use crate::core::elements::{
    ComposedGestureEvent, ElementJsCallbackEmitter, ElementOnFocus, ElementOnGesture,
    ElementOnGestureContext, ElementOnIme, ElementOnImeContext, ElementOnKeyboard,
    ElementOnKeyboardContext, ElementOnUpdate, ElementTrace,
};
use crate::core::event::AppImeEvent;
use crate::core::js_command::{AnyJsCommand, FocusableJsCommand, IntoAnyJsCommand};
use crate::core::js_command::helpers::build_key_event_object;
use crate::core::keyboard::{AppKeyEvent, KeyEventType};
use crate::core::text::TextEditingController;
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
    controller_obj: JsObject,
    pub(crate) font_size: f64,
    pub(crate) color: Option<Color>,
    pub(crate) cursor_color: Option<Color>,
    pub(crate) placeholder: Option<String>,
    pub(crate) placeholder_color: Option<Color>,
    pub(crate) multiline: bool,
    pub(crate) cached_layout: Option<TextLayoutData>,
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
        Self::new(JsObject::with_null_proto())
    }
}

impl EditableTextElement {
    pub fn new(controller_obj: JsObject) -> Self {
        EditableTextElement {
            controller_obj,
            font_size: 14.0,
            color: None,
            cursor_color: None,
            placeholder: None,
            placeholder_color: None,
            multiline: false,
            cached_layout: None,
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

    pub(crate) fn controller(&self) -> boa_engine::object::Ref<'_, TextEditingController> {
        BoaOpaque::<TextEditingController>::wrap(&self.controller_obj)
            .expect("controller is always a valid TextControllerHandle")
    }

    pub(crate) fn controller_mut(&self) -> boa_engine::object::RefMut<'_, TextEditingController> {
        BoaOpaque::<TextEditingController>::wrap_mut(&self.controller_obj)
            .expect("controller is always a valid TextControllerHandle")
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

    pub(crate) fn composition_display_text(&self) -> String {
        self.controller().composition_display_text()
    }

    fn handle_key_event(
        &self,
        key: &str,
        ctrl: bool,
        meta: bool,
        shift: bool,
        nav_info: Option<&LineNavInfo>,
    ) -> bool {
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

        let handled = match key {
            "Backspace" => {
                if has_sel {
                    let (s, e) = if anchor <= end { (anchor, end) } else { (end, anchor) };
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
                    let (s, e) = if anchor <= end { (anchor, end) } else { (end, anchor) };
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
                    if !has_sel { new_anchor = cursor; }
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
                    if !has_sel { new_anchor = cursor; }
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
            "ArrowUp" if self.multiline => {
                if let Some(info) = nav_info {
                    let target_byte = compute_vertical_target(info, &full, -1);
                    if shift {
                        if !has_sel { new_anchor = cursor; }
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
            "ArrowDown" if self.multiline => {
                if let Some(info) = nav_info {
                    let target_byte = compute_vertical_target(info, &full, 1);
                    if shift {
                        if !has_sel { new_anchor = cursor; }
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
                let target = if self.multiline {
                    if let Some(info) = nav_info {
                        let line_start = info.line_start_chars[info.current_line];
                        char_to_byte_offset(&full, line_start)
                    } else {
                        return false;
                    }
                } else {
                    0
                };
                if shift {
                    if !has_sel { new_anchor = cursor; }
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
                let target = if self.multiline {
                    if let Some(info) = nav_info {
                        let line_end = info.line_end_chars[info.current_line];
                        char_to_byte_offset(&full, line_end)
                    } else {
                        return false;
                    }
                } else {
                    len
                };
                if shift {
                    if !has_sel { new_anchor = cursor; }
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
            "Enter" => {
                if self.multiline {
                    if has_sel {
                        let (s, e) = if anchor <= end { (anchor, end) } else { (end, anchor) };
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
                        let (s, e) = if anchor <= end { (anchor, end) } else { (end, anchor) };
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

        handled
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

fn compute_vertical_target(info: &LineNavInfo, full: &str, direction: i32) -> usize {
    let current_line = info.current_line;
    let cursor_x = info.cursor_xy.0;
    let num_lines = info.line_start_chars.len();

    let target_line = if direction < 0 {
        current_line.saturating_sub(1)
    } else {
        (current_line + 1).min(num_lines - 1)
    };

    if target_line == current_line {
        return char_to_byte_offset(full, info.line_start_chars[current_line]);
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

    char_to_byte_offset(full, target_char)
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
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        match key.to_std_string_escaped().as_str() {
            "controller" => {
                if let Some(obj) = value.as_object() {
                    if BoaOpaque::<TextEditingController>::wrap(&obj).is_some() {
                        self.controller_obj = obj;
                    }
                }
            }
            "fontSize" => {
                self.font_size = value.as_number().unwrap_or(14.0);
            }
            "color" => {
                self.color = extract_color(value, _ctx);
            }
            "cursorColor" => {
                self.cursor_color = extract_color(value, _ctx);
            }
            "placeholder" => {
                self.placeholder = value.as_string().map(|s| s.to_std_string_escaped());
            }
            "placeholderColor" => {
                self.placeholder_color = extract_color(value, _ctx);
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
            "controller" => {}
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
        let full = {
            let c = self.controller();
            c.text()
        };
        match event {
            ComposedGestureEvent::PointerDown { local_position } => {
                cx.request_own_focus();
                let char_idx = self.char_index_at(local_position);
                let byte_pos = char_to_byte_offset(&full, char_idx);
                let mut c = self.controller_mut();
                c.set_cursor_position(byte_pos);
                c.set_selection(byte_pos, byte_pos);
                drop(c);
                cx.request_redraw();
            }
            ComposedGestureEvent::PointerMove { local_position } => {
                let char_idx = self.char_index_at(local_position);
                let byte_pos = char_to_byte_offset(&full, char_idx);
                let mut c = self.controller_mut();
                let anchor = c.selection_anchor();
                let sel_end = c.selection_end();
                if byte_pos != sel_end {
                    c.set_selection(anchor, byte_pos);
                    c.set_cursor_position(byte_pos);
                    drop(c);
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

        let (prev_text, prev_cursor, prev_anchor, prev_end, cursor_char) = {
            let c = self.controller();
            let text = c.text();
            let cursor_char = byte_to_char_offset(&text, c.cursor_position());
            (text, c.cursor_position(), c.selection_anchor(), c.selection_end(), cursor_char)
        };

        let nav_info = self.cached_layout.as_ref().map(|ld| {
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

            let c = self.controller();
            let new_text = c.text();
            if new_text != prev_text {
                let enter = event.key == "Enter" && !self.multiline;
                cx.push_js_command(EditableTextNotification::TextChanged {
                    text: new_text,
                    enter,
                });
            }
            let cursor = c.cursor_position();
            if cursor != prev_cursor {
                cx.push_js_command(EditableTextNotification::CursorChanged {
                    position: cursor,
                });
            }
            let anchor = c.selection_anchor();
            let end = c.selection_end();
            if anchor != prev_anchor || end != prev_end {
                cx.push_js_command(EditableTextNotification::SelectionChanged {
                    anchor,
                    end,
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
                let mut c = self.controller_mut();
                c.start_composition();
                drop(c);
                cx.push_js_command(EditableTextNotification::CompositionStarted);
                cx.request_redraw();
            }
            AppImeEvent::CompositionUpdate { text, .. } => {
                let mut c = self.controller_mut();
                c.update_composition(text.clone());
                drop(c);
                cx.push_js_command(EditableTextNotification::CompositionUpdated {
                    text: text.clone(),
                });
                cx.request_redraw();
            }
            AppImeEvent::CompositionEnd { text } => {
                let mut c = self.controller_mut();
                if c.finish_composition().is_some() {
                    let start = c.composing_start().min(c.full_len());
                    c.insert_str_at(start, text);
                    c.set_cursor_position(start + text.len());
                    c.clear_selection();

                    let new_text = c.text();
                    let cursor = c.cursor_position();
                    drop(c);
                    cx.push_js_command(EditableTextNotification::CompositionEnded {
                        text: text.clone(),
                    });
                    cx.push_js_command(EditableTextNotification::TextChanged {
                        text: new_text,
                        enter: false,
                    });
                    cx.push_js_command(EditableTextNotification::CursorChanged {
                        position: cursor,
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
