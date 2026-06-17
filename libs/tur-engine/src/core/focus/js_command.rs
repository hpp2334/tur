use std::rc::Rc;

use crate::core::js_command::{AnyJsCommand, IntoAnyJsCommand};
use crate::core::keyboard::Modifiers;

#[derive(Clone)]
pub enum FocusableJsCommand {
    KeyDown { key: String, code: String, modifiers: Modifiers },
    KeyUp { key: String, code: String, modifiers: Modifiers },
    Focus,
    Blur,
}

impl IntoAnyJsCommand for FocusableJsCommand {
    fn into_any_js_command(self) -> AnyJsCommand {
        AnyJsCommand(Rc::new(self))
    }
}
