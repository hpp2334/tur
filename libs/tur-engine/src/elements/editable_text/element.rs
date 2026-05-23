use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsString, JsValue};
use tur_shared::Color;

use crate::core::bridge::color::extract_color;
use crate::core::elements::{
    ComposedGestureEvent, ElementJsCallbackEmitter, ElementOnFocus, ElementOnGesture,
    ElementOnGestureContext, ElementOnIme, ElementOnImeContext, ElementOnKeyboard,
    ElementOnKeyboardContext, ElementOnUpdate, ElementTrace,
};
use crate::core::event::AppImeEvent;
use crate::core::js_command::{
    AnyJsCommand, EditableTextJsCommand, FocusableJsCommand,
};
use crate::core::js_command::helpers::build_key_event_object;
use crate::core::keyboard::AppKeyEvent;
use crate::elements::text::span_data::SpanData;
use crate::elements::text::text_layout::TextLayoutData;

fn extract_callable(value: &JsValue) -> Option<JsFunction> {
    value.as_object().and_then(JsFunction::from_object)
}

pub struct EditableTextElement {
    pub(crate) spans: Vec<SpanData>,
    pub(crate) font_size: f64,
    pub(crate) color: Option<Color>,
    pub(crate) placeholder: Option<String>,
    pub(crate) placeholder_color: Option<Color>,
    pub(crate) multiline: bool,
    pub(crate) selection_start: Option<usize>,
    pub(crate) selection_end: Option<usize>,
    pub(crate) selection_color: Option<Color>,
    pub(crate) cursor_position: Option<usize>,
    pub(crate) cursor_color: Option<Color>,
    pub(crate) composition_start: Option<usize>,
    pub(crate) composition_end: Option<usize>,
    pub(crate) cached_layout: Option<TextLayoutData>,
    on_key_down: Option<JsFunction>,
    on_key_up: Option<JsFunction>,
    on_focus: Option<JsFunction>,
    on_blur: Option<JsFunction>,
    on_pointer_down: Option<JsFunction>,
    on_pointer_move: Option<JsFunction>,
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
            placeholder: None,
            placeholder_color: None,
            multiline: false,
            selection_start: None,
            selection_end: None,
            selection_color: None,
            cursor_position: None,
            cursor_color: None,
            composition_start: None,
            composition_end: None,
            cached_layout: None,
            on_key_down: None,
            on_key_up: None,
            on_focus: None,
            on_blur: None,
            on_pointer_down: None,
            on_pointer_move: None,
            on_composition_start: None,
            on_composition_update: None,
            on_composition_end: None,
        }
    }
}

impl ElementTrace for EditableTextElement {
    fn trace_label(&self) -> String {
        let text: String = self.spans.iter().map(|s| s.text.as_str()).collect();
        if text.is_empty() {
            String::new()
        } else {
            format!("\"{}\"", if text.len() > 20 { &text[..20] } else { &text })
        }
    }
}

impl ElementOnUpdate for EditableTextElement {
    fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue) {
        match key.to_std_string_escaped().as_str() {
            "spans" => {
                self.spans = crate::elements::text::span_data::extract_spans_from_js(value, ctx);
            }
            "fontSize" => {
                self.font_size = value.as_number().unwrap_or(14.0);
            }
            "color" => {
                self.color = extract_color(value, ctx);
            }
            "placeholder" => {
                self.placeholder = value
                    .as_string()
                    .map(|s| s.to_std_string_escaped());
            }
            "placeholderColor" => {
                self.placeholder_color = extract_color(value, ctx);
            }
            "multiline" => {
                self.multiline = value.as_boolean().unwrap_or(value.to_boolean());
            }
            "selectionStart" => {
                self.selection_start = value
                    .as_number()
                    .map(|n| n as usize);
            }
            "selectionEnd" => {
                self.selection_end = value
                    .as_number()
                    .map(|n| n as usize);
            }
            "selectionColor" => {
                self.selection_color = extract_color(value, ctx);
            }
            "cursorPosition" => {
                self.cursor_position = value
                    .as_number()
                    .map(|n| n as usize);
            }
            "cursorColor" => {
                self.cursor_color = extract_color(value, ctx);
            }
            "compositionStart" => {
                self.composition_start = value
                    .as_number()
                    .map(|n| n as usize);
            }
            "compositionEnd" => {
                self.composition_end = value
                    .as_number()
                    .map(|n| n as usize);
            }
            "onKeyDown" => {
                self.on_key_down = extract_callable(value);
            }
            "onKeyUp" => {
                self.on_key_up = extract_callable(value);
            }
            "onFocus" => {
                self.on_focus = extract_callable(value);
            }
            "onBlur" => {
                self.on_blur = extract_callable(value);
            }
            "onPointerDown" => {
                self.on_pointer_down = extract_callable(value);
            }
            "onPointerMove" => {
                self.on_pointer_move = extract_callable(value);
            }
            "onCompositionStart" => {
                self.on_composition_start = extract_callable(value);
            }
            "onCompositionUpdate" => {
                self.on_composition_update = extract_callable(value);
            }
            "onCompositionEnd" => {
                self.on_composition_end = extract_callable(value);
            }
            _ => {}
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        match key.to_std_string_escaped().as_str() {
            "spans" => self.spans.clear(),
            "fontSize" => self.font_size = 14.0,
            "color" => self.color = None,
            "placeholder" => self.placeholder = None,
            "placeholderColor" => self.placeholder_color = None,
            "multiline" => self.multiline = false,
            "selectionStart" => self.selection_start = None,
            "selectionEnd" => self.selection_end = None,
            "selectionColor" => self.selection_color = None,
            "cursorPosition" => self.cursor_position = None,
            "cursorColor" => self.cursor_color = None,
            "compositionStart" => self.composition_start = None,
            "compositionEnd" => self.composition_end = None,
            "onKeyDown" => self.on_key_down = None,
            "onKeyUp" => self.on_key_up = None,
            "onFocus" => self.on_focus = None,
            "onBlur" => self.on_blur = None,
            "onPointerDown" => self.on_pointer_down = None,
            "onPointerMove" => self.on_pointer_move = None,
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
        match event {
            ComposedGestureEvent::PointerDown { local_position } => {
                cx.request_own_focus();
                cx.push_js_command(EditableTextJsCommand::PointerDown {
                    x: local_position.x,
                    y: local_position.y,
                });
                cx.request_redraw();
            }
            ComposedGestureEvent::PointerMove { local_position } => {
                cx.push_js_command(EditableTextJsCommand::PointerMove {
                    x: local_position.x,
                    y: local_position.y,
                });
                cx.request_redraw();
            }
        }
    }
}

impl ElementOnKeyboard for EditableTextElement {
    fn on_keyboard_event(
        &mut self,
        _cx: &mut ElementOnKeyboardContext,
        _event: &AppKeyEvent,
    ) {
        // Key events are dispatched via FocusableJsCommand by the keyboard handler.
        // No internal editing logic here.
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
                cx.push_js_command(EditableTextJsCommand::CompositionStart);
                cx.request_redraw();
            }
            AppImeEvent::CompositionUpdate { text, .. } => {
                cx.push_js_command(EditableTextJsCommand::CompositionUpdate {
                    text: text.clone(),
                });
                cx.request_redraw();
            }
            AppImeEvent::CompositionEnd { text } => {
                cx.push_js_command(EditableTextJsCommand::CompositionEnd {
                    text: text.clone(),
                });
                cx.request_redraw();
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

        if let Some(c) = command.downcast_ref::<EditableTextJsCommand>() {
            match c {
                EditableTextJsCommand::PointerDown { x, y } => {
                    self.on_pointer_down.as_ref().map(|h| {
                        (h.clone(), vec![JsValue::from(*x), JsValue::from(*y)])
                    })
                }
                EditableTextJsCommand::PointerMove { x, y } => {
                    self.on_pointer_move.as_ref().map(|h| {
                        (h.clone(), vec![JsValue::from(*x), JsValue::from(*y)])
                    })
                }
                EditableTextJsCommand::CompositionStart => {
                    self.on_composition_start.as_ref().map(|h| (h.clone(), vec![]))
                }
                EditableTextJsCommand::CompositionUpdate { text } => {
                    self.on_composition_update.as_ref().map(|h| {
                        let text_val = JsValue::from(js_string!(text.as_str()));
                        (h.clone(), vec![text_val])
                    })
                }
                EditableTextJsCommand::CompositionEnd { text } => {
                    self.on_composition_end.as_ref().map(|h| {
                        let text_val = JsValue::from(js_string!(text.as_str()));
                        (h.clone(), vec![text_val])
                    })
                }
            }
        } else if let Some(c) = command.downcast_ref::<FocusableJsCommand>() {
            match c {
                FocusableJsCommand::KeyDown {
                    key,
                    code,
                    modifiers,
                } => {
                    self.on_key_down.as_ref().map(|h| {
                        let event_obj = build_key_event_object(key, code, modifiers, context);
                        (h.clone(), vec![event_obj])
                    })
                }
                FocusableJsCommand::KeyUp {
                    key,
                    code,
                    modifiers,
                } => {
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
