use std::rc::Rc;

use crate::core::js_command::{AnyJsCommand, IntoAnyJsCommand};

#[derive(Clone)]
pub enum PointerInteractJsCommand {
    Click { x: f64, y: f64 },
    PointerEnter,
    PointerExit,
}

impl IntoAnyJsCommand for PointerInteractJsCommand {
    fn into_any_js_command(self) -> AnyJsCommand {
        AnyJsCommand(Rc::new(self))
    }
}
