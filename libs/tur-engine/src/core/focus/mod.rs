pub mod helper;
pub mod js_command;

pub use js_command::FocusableJsCommand;

use boa_engine::{Context, JsValue};

use crate::core::element::ElementNodeId;
use crate::core::js_command::JsCommandQueue;
use crate::core::widget::callback::EventArg;

// ---------------------------------------------------------------------------
// Focus event payloads — JS callback arguments for focus / blur.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FocusEvent;

#[derive(Clone)]
pub struct BlurEvent;

impl EventArg for FocusEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        Vec::new()
    }
}

impl EventArg for BlurEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        Vec::new()
    }
}

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
