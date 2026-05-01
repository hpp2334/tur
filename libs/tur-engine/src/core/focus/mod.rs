pub mod helper;

use crate::core::element::ElementNodeId;
use crate::core::js_command::{FocusableJsCommand, JsCommandQueue};

#[derive(Debug)]
pub struct FocusManager {
    focused_id: Option<ElementNodeId>,
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusManager {
    pub fn new() -> Self {
        Self { focused_id: None }
    }

    pub fn focused(&self) -> Option<ElementNodeId> {
        self.focused_id
    }

    pub fn is_focused(&self, id: ElementNodeId) -> bool {
        self.focused_id == Some(id)
    }

    pub fn set_focus(&mut self, new_id: ElementNodeId, queue: &mut JsCommandQueue) {
        let old = self.focused_id.replace(new_id);
        if let Some(old) = old {
            if old != new_id {
                queue.push(old, FocusableJsCommand::Blur);
            }
        }
        queue.push(new_id, FocusableJsCommand::Focus);
    }

    pub fn clear_focus(&mut self, queue: &mut JsCommandQueue) {
        if let Some(old) = self.focused_id.take() {
            queue.push(old, FocusableJsCommand::Blur);
        }
    }
}
