use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsString, JsValue};

use crate::core::elements::{ElementJsCallbackEmitter, ElementOnUpdate, ElementTrace};
use crate::core::js_command::{AnyJsCommand, PointerInteractJsCommand};

fn extract_callable(value: &JsValue) -> Option<JsFunction> {
    value.as_object().and_then(JsFunction::from_object)
}

pub struct PointerInteractElement {
    on_click: Option<JsFunction>,
}

impl Default for PointerInteractElement {
    fn default() -> Self {
        Self::new()
    }
}

impl PointerInteractElement {
    pub fn new() -> Self {
        Self { on_click: None }
    }

    pub fn has_on_click(&self) -> bool {
        self.on_click.is_some()
    }
}

impl ElementTrace for PointerInteractElement {}

impl ElementOnUpdate for PointerInteractElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "onClick" {
            self.on_click = extract_callable(value);
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
        }
    }
}
