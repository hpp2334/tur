use std::rc::Rc;

use crate::core::js_command::{AnyJsCommand, IntoAnyJsCommand};

#[derive(Clone)]
pub enum LazyListJsCommand {
    VisibleRangeDidChange { start_index: u64, end_index: u64 },
    ScrollDidUpdate,
}

impl IntoAnyJsCommand for LazyListJsCommand {
    fn into_any_js_command(self) -> AnyJsCommand {
        AnyJsCommand(Rc::new(self))
    }
}
