use std::rc::Rc;

use crate::core::js_command::{AnyJsCommand, IntoAnyJsCommand};

#[derive(Clone)]
pub enum ScrollViewJsCommand {
    ScrollDidUpdate,
}

impl IntoAnyJsCommand for ScrollViewJsCommand {
    fn into_any_js_command(self) -> AnyJsCommand {
        AnyJsCommand(Rc::new(self))
    }
}
