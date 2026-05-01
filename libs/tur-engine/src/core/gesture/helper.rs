use crate::core::js_command::{AnyJsCommand, PointerInteractJsCommand};

pub fn make_click_command(x: f64, y: f64) -> AnyJsCommand {
    AnyJsCommand(std::rc::Rc::new(PointerInteractJsCommand::Click { x, y }))
}
