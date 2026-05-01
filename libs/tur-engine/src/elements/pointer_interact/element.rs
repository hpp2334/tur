use boa_engine::object::JsObject;
use boa_engine::{Context, JsString, JsValue};

use crate::core::elements::{ElementJsCommandEmitter, ElementOnUpdate, ElementTrace};
use crate::core::js_command::{AnyJsCommand, PointerInteractJsCommand};

fn extract_callable(value: &JsValue) -> Option<JsObject> {
    value.as_object().and_then(|o| {
        if o.is_callable() {
            Some(o.clone())
        } else {
            None
        }
    })
}

pub struct PointerInteractElement {
    on_click: Option<JsObject>,
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

impl ElementJsCommandEmitter for PointerInteractElement {
    fn flush_js_command(&mut self, command: AnyJsCommand, context: &mut Context) {
        let Some(c) = command.downcast_ref::<PointerInteractJsCommand>() else {
            return;
        };
        match c {
            PointerInteractJsCommand::Click { x, y } => {
                if let Some(ref handler) = self.on_click {
                    let _ = handler.call(
                        &JsValue::undefined(),
                        &[JsValue::from(*x), JsValue::from(*y)],
                        context,
                    );
                }
            }
        }
    }
}
