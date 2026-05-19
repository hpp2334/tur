use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsString, JsValue};

use crate::core::elements::{ElementJsCallbackEmitter, ElementOnFocus, ElementOnUpdate, ElementTrace};
use crate::core::js_command::{AnyJsCommand, FocusableJsCommand};
use crate::core::js_command::helpers::build_key_event_object;

fn extract_callable(value: &JsValue) -> Option<JsFunction> {
    value.as_object().and_then(JsFunction::from_object)
}

pub struct FocusableElement {
    on_key_down: Option<JsFunction>,
    on_key_up: Option<JsFunction>,
    on_focus: Option<JsFunction>,
    on_blur: Option<JsFunction>,
}

impl Default for FocusableElement {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusableElement {
    pub fn new() -> Self {
        Self {
            on_key_down: None,
            on_key_up: None,
            on_focus: None,
            on_blur: None,
        }
    }
}

impl ElementTrace for FocusableElement {}

impl ElementOnUpdate for FocusableElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        match key.to_std_string_escaped().as_str() {
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
            _ => {}
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        match key.to_std_string_escaped().as_str() {
            "onKeyDown" => self.on_key_down = None,
            "onKeyUp" => self.on_key_up = None,
            "onFocus" => self.on_focus = None,
            "onBlur" => self.on_blur = None,
            _ => {}
        }
    }
}

impl ElementOnFocus for FocusableElement {}

impl ElementJsCallbackEmitter for FocusableElement {
    fn emit_js_callback(
        &self,
        context: &mut Context,
        command: AnyJsCommand,
    ) -> Option<(JsFunction, Vec<JsValue>)> {
        let c = command.downcast_ref::<FocusableJsCommand>()?;
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
    }
}
