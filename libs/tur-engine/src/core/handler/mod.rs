use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use crate::core::edgy_event::PendingMutationInvocationQueue;
use crate::core::elements::NodeTreeData;
use crate::core::event::queue::AppEventQueue;
use crate::core::focus::FocusManager;
use crate::core::gesture::GestureEventComposer;
use crate::core::render::Renderer;

pub trait AppHandler {
    fn handle_event(&mut self, cx: &mut HandlerContext, event: &crate::core::event::AppEvent);
}

pub struct HandlerContext<'a> {
    pub element_tree: &'a mut NodeTreeData,
    pub focus_manager: &'a mut FocusManager,
    pub mutation_queue: &'a mut PendingMutationInvocationQueue,
    pub event_queue: &'a mut AppEventQueue,
    pub gesture_composer: &'a mut GestureEventComposer,
    pub renderer: &'a mut dyn Renderer,
    pub size: &'a mut (f64, f64),
    pub(crate) needs_draw: &'a Cell<bool>,
    /// Slot for `AppEvent::ClipboardWrite` payloads — `ClipboardWriteHandler`
    /// pushes the text here, and the embedder drains it via
    /// `TurApp::take_clipboard_write()` once per frame.
    pub(crate) pending_clipboard_write: Rc<RefCell<Option<String>>>,
}

impl<'a> HandlerContext<'a> {
    pub fn request_draw(&self) {
        self.needs_draw.set(true);
    }

    /// Capture an `AppEvent::ClipboardWrite` payload so the embedder can
    /// drain it on the next frame poll. Multiple writes between polls keep
    /// the latest (matches typical clipboard semantics).
    pub fn push_clipboard_write(&self, text: String) {
        *self.pending_clipboard_write.borrow_mut() = Some(text);
    }
}
