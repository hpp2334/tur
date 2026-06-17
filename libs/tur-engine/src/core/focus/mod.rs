pub mod helper;

use boa_engine::{Context, JsValue};

use crate::core::element::ElementNodeId;
use crate::core::edgy_event::EventArg;

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

// ---------------------------------------------------------------------------
// FocusChange — a deferred focus/blur notification. `set_focus` / `clear_focus`
// only update `focused_id` and record a pending change; a flush step
// (`flush_focus_notifications` in the app loop) resolves each pending id to its
// `EdgyMutation` via the element tree and pushes the invocation. This keeps
// `set_focus` free of tree/queue borrows so it can be called from inside
// element gesture handlers (which already hold the tree).
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub(crate) enum FocusChange {
    Focus(ElementNodeId),
    Blur(ElementNodeId),
}

#[derive(Debug)]
pub struct FocusManager {
    focused_id: Option<ElementNodeId>,
    pending: Vec<FocusChange>,
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusManager {
    pub fn new() -> Self {
        Self { focused_id: None, pending: Vec::new() }
    }

    pub fn focused(&self) -> Option<ElementNodeId> {
        self.focused_id
    }

    pub fn is_focused(&self, id: ElementNodeId) -> bool {
        self.focused_id == Some(id)
    }

    pub fn set_focus(&mut self, new_id: ElementNodeId) {
        let old = self.focused_id.replace(new_id);
        if let Some(old) = old {
            if old != new_id {
                self.pending.push(FocusChange::Blur(old));
            }
        }
        self.pending.push(FocusChange::Focus(new_id));
    }

    pub fn clear_focus(&mut self) {
        if let Some(old) = self.focused_id.take() {
            self.pending.push(FocusChange::Blur(old));
        }
    }

    pub(crate) fn drain_pending(&mut self) -> Vec<FocusChange> {
        std::mem::take(&mut self.pending)
    }
}
