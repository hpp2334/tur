use crate::core::js_command::{AnyJsCommand, FocusableJsCommand};
use crate::core::keyboard::AppKeyEvent;

pub fn make_key_down_command(event: &AppKeyEvent) -> AnyJsCommand {
    AnyJsCommand(std::rc::Rc::new(FocusableJsCommand::KeyDown {
        key: event.key.clone(),
        code: event.code.clone(),
        modifiers: event.modifiers.clone(),
    }))
}

pub fn make_key_up_command(event: &AppKeyEvent) -> AnyJsCommand {
    AnyJsCommand(std::rc::Rc::new(FocusableJsCommand::KeyUp {
        key: event.key.clone(),
        code: event.code.clone(),
        modifiers: event.modifiers.clone(),
    }))
}
