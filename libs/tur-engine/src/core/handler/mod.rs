pub mod pointer_focus;

use crate::core::event::AppEvent;
use crate::core::focus::FocusManager;
use crate::core::js_command::JsCommandQueue;
use crate::core::elements::ElementTree;

pub trait AppHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &AppEvent);
}

pub struct HandlerContext<'a> {
    pub element_tree: &'a ElementTree,
    pub focus_manager: &'a mut FocusManager,
    pub js_command_queue: &'a mut JsCommandQueue,
}
