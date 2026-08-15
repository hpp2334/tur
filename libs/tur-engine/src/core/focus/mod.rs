pub mod focusable;
pub mod helper;

pub use focusable::Focusable;

use boa_engine::{Context, JsValue};

use crate::core::edgy::mutation::IntoJsArgs;
use crate::core::element::ElementNodeId;

// ---------------------------------------------------------------------------
// Focus event payloads — JS callback arguments for focus / blur.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FocusEvent;

#[derive(Clone)]
pub struct BlurEvent;

impl IntoJsArgs for FocusEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        Vec::new()
    }
}

impl IntoJsArgs for BlurEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// FocusChange — a deferred focus/blur notification. `set_focus` / `clear_focus`
// only update `focused_id` and record a pending change; a flush step
// (`flush_focus_notifications` in the app loop) resolves each pending id to its
// `MutationHandle` via the element tree and pushes the invocation. This keeps
// `set_focus` free of tree/queue borrows so it can be called from inside
// element gesture handlers (which already hold the tree).
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub enum FocusChange {
    Focus(ElementNodeId),
    Blur(ElementNodeId),
}

impl FocusChange {
    pub fn id(&self) -> ElementNodeId {
        match self {
            FocusChange::Focus(id) | FocusChange::Blur(id) => *id,
        }
    }
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
        Self {
            focused_id: None,
            pending: Vec::new(),
        }
    }

    pub fn focused(&self) -> Option<ElementNodeId> {
        self.focused_id
    }

    pub fn is_focused(&self, id: ElementNodeId) -> bool {
        self.focused_id == Some(id)
    }

    pub fn set_focus(&mut self, new_id: ElementNodeId) {
        let old = self.focused_id.replace(new_id);
        if let Some(old) = old
            && old != new_id
        {
            self.pending.push(FocusChange::Blur(old));
        }
        self.pending.push(FocusChange::Focus(new_id));
    }

    pub fn clear_focus(&mut self) {
        if let Some(old) = self.focused_id.take() {
            self.pending.push(FocusChange::Blur(old));
        }
    }

    pub fn drain_pending(&mut self) -> Vec<FocusChange> {
        std::mem::take(&mut self.pending)
    }

    /// Resolve pending focus/blur notifications into `MutationHandle`s and push
    /// them onto the pending-mutation queue. Each pending id is looked up in
    /// the element tree; if it resolves to a `Focusable` element with an
    /// `on_focus` / `on_blur` mutation, the invocation is enqueued.
    ///
    /// This is the flush step paired with the deferred `set_focus` /
    /// `clear_focus`: those only record a pending `FocusChange` (so they stay
    /// free of tree/queue borrows and can run inside element gesture
    /// handlers); this method resolves the changes once per frame.
    ///
    /// Returns the list of resolved `(id, focused)` pairs. Callers use this
    /// to dispatch Rust-level `on_focus_changed` callbacks (e.g. to spawn
    /// async tasks tied to focus state) and to force a paint so
    /// that focus-sensitive paint effects update immediately.
    /// Resolve pending focus/blur changes to mutations + lifecycle
    /// notifications. Pending changes may reference elements in any of the
    /// instance's view-root trees (node ids are unique instance-wide), so
    /// each change resolves against the tree that owns its element id.
    pub fn flush_pending(
        &mut self,
        trees: &[crate::core::elements::NodeTree],
        queue: &mut crate::core::edgy::mutation::PendingMutationInvocationQueue,
    ) -> Vec<(ElementNodeId, bool)> {
        let changes = self.drain_pending();
        if changes.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::new();
        for change in changes {
            let id = change.id();
            let Some(tree) = trees.iter().find(|t| t.view_root() == id.root()) else {
                continue;
            };
            let tree = tree.borrow();
            let Some(node) = tree.get_element(id) else {
                continue;
            };
            let Some(ref element) = node.element else {
                continue;
            };
            let Some(focusable) = focusable::as_focusable(element) else {
                continue;
            };
            match change {
                FocusChange::Focus(_) => {
                    if let Some(m) = focusable.on_focus_mutation() {
                        queue.push(m, FocusEvent);
                    }
                    result.push((id, true));
                }
                FocusChange::Blur(_) => {
                    if let Some(m) = focusable.on_blur_mutation() {
                        queue.push(m, BlurEvent);
                    }
                    result.push((id, false));
                }
            }
        }
        result
    }
}
