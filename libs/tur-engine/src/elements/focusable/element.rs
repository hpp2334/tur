use boa_engine::object::JsObject;
use boa_engine::{Context, JsString, JsValue};

use crate::core::elements::{ElementJsCommandEmitter, ElementOnFocus, ElementOnUpdate, ElementTrace};
use crate::core::js_command::{AnyJsCommand, FocusableJsCommand};
use crate::core::js_command::helpers::build_key_event_object;

fn extract_callable(value: &JsValue) -> Option<JsObject> {
    value.as_object().and_then(|o| {
        if o.is_callable() {
            Some(o.clone())
        } else {
            None
        }
    })
}

pub struct FocusableElement {
    on_key_down: Option<JsObject>,
    on_key_up: Option<JsObject>,
    on_focus: Option<JsObject>,
    on_blur: Option<JsObject>,
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
}

impl ElementOnFocus for FocusableElement {}

impl ElementJsCommandEmitter for FocusableElement {
    fn flush_js_command(&mut self, command: AnyJsCommand, context: &mut Context) {
        let Some(c) = command.downcast_ref::<FocusableJsCommand>() else {
            return;
        };
        match c {
            FocusableJsCommand::KeyDown {
                key,
                code,
                modifiers,
            } => {
                if let Some(ref handler) = self.on_key_down {
                    let event_obj = build_key_event_object(key, code, modifiers, context);
                    let _ = handler.call(&JsValue::undefined(), &[event_obj], context);
                }
            }
            FocusableJsCommand::KeyUp {
                key,
                code,
                modifiers,
            } => {
                if let Some(ref handler) = self.on_key_up {
                    let event_obj = build_key_event_object(key, code, modifiers, context);
                    let _ = handler.call(&JsValue::undefined(), &[event_obj], context);
                }
            }
            FocusableJsCommand::Focus => {
                if let Some(ref handler) = self.on_focus {
                    let _ = handler.call(&JsValue::undefined(), &[], context);
                }
            }
            FocusableJsCommand::Blur => {
                if let Some(ref handler) = self.on_blur {
                    let _ = handler.call(&JsValue::undefined(), &[], context);
                }
            }
        }
    }
}
