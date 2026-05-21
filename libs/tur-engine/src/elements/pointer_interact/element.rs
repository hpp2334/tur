use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsString, JsValue};
use num_traits::FromPrimitive;
use tur_shared::HitTestBehavior;

use crate::core::elements::{ElementJsCallbackEmitter, ElementOnUpdate, ElementTrace};
use crate::core::js_command::{AnyJsCommand, PointerInteractJsCommand};

fn extract_callable(value: &JsValue) -> Option<JsFunction> {
    value.as_object().and_then(JsFunction::from_object)
}

pub struct PointerInteractElement {
    on_click: Option<JsFunction>,
    on_pointer_enter: Option<JsFunction>,
    on_pointer_exit: Option<JsFunction>,
    behavior: HitTestBehavior,
}

impl Default for PointerInteractElement {
    fn default() -> Self {
        Self::new()
    }
}

impl PointerInteractElement {
    pub fn new() -> Self {
        Self {
            on_click: None,
            on_pointer_enter: None,
            on_pointer_exit: None,
            behavior: HitTestBehavior::default(),
        }
    }

    pub fn has_on_click(&self) -> bool {
        self.on_click.is_some()
    }

    pub fn has_pointer_region_callbacks(&self) -> bool {
        self.on_pointer_enter.is_some() || self.on_pointer_exit.is_some()
    }

    pub fn is_click_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque && self.on_click.is_some()
    }

    pub fn is_pointer_region_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque
            && (self.on_pointer_enter.is_some() || self.on_pointer_exit.is_some())
    }
}

impl ElementTrace for PointerInteractElement {}

impl ElementOnUpdate for PointerInteractElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "onClick" {
            self.on_click = extract_callable(value);
        } else if *key == "onPointerEnter" {
            self.on_pointer_enter = extract_callable(value);
        } else if *key == "onPointerExit" {
            self.on_pointer_exit = extract_callable(value);
        } else if *key == "behavior" {
            if let Some(n) = value.as_number() {
                if let Some(b) = HitTestBehavior::from_u8(n as u8) {
                    self.behavior = b;
                }
            }
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        if key.to_std_string_escaped().as_str() == "onClick" {
            self.on_click = None;
        }
    }
}

impl ElementJsCallbackEmitter for PointerInteractElement {
    fn emit_js_callback(
        &self,
        _context: &mut Context,
        command: AnyJsCommand,
    ) -> Option<(JsFunction, Vec<JsValue>)> {
        let c = command.downcast_ref::<PointerInteractJsCommand>()?;
        match c {
            PointerInteractJsCommand::Click { x, y } => {
                self.on_click.as_ref().map(|h| {
                    (h.clone(), vec![JsValue::from(*x), JsValue::from(*y)])
                })
            }
            PointerInteractJsCommand::PointerEnter => {
                self.on_pointer_enter.as_ref().map(|h| {
                    (h.clone(), vec![])
                })
            }
            PointerInteractJsCommand::PointerExit => {
                self.on_pointer_exit.as_ref().map(|h| {
                    (h.clone(), vec![])
                })
            }
        }
    }
}
