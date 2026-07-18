use std::cell::Cell;
use std::rc::Rc;

use crate::core::async_::AsyncExecutor;
use crate::core::capability::Capabilities;
use crate::core::mutation::PendingMutationInvocationQueue;
use crate::core::elements::NodeTreeData;
use crate::core::event::queue::{AppEventQueue, PlatformEventQueue};
use crate::core::event::{AppEvent, PlatformEvent};
use crate::core::focus::FocusManager;
use crate::core::render::Renderer;

/// Element-/app-level event handler. Each registered handler is invoked for
/// every drained event. The two methods default to no-ops so a handler only
/// overrides the kind it cares about:
///
/// - [`Self::handle_platform_event`] — input from the platform/embedder
///   (`Resize`, `Gesture`, `Wheel`, `Key`, `Ime`, `ClipboardPaste`).
/// - [`Self::handle_app_event`] — engine-internal bus (`ScrollTo`,
///   `ScrollOverscroll`, `ClipboardWrite`).
pub trait AppHandler {
    fn handle_platform_event(&mut self, _cx: &mut HandlerContext, _event: &PlatformEvent) {}
    fn handle_app_event(&mut self, _cx: &mut HandlerContext, _event: &AppEvent) {}
}

pub struct HandlerContext<'a> {
    pub element_tree: &'a mut NodeTreeData,
    pub focus_manager: &'a mut FocusManager,
    pub mutation_queue: &'a mut PendingMutationInvocationQueue,
    pub platform_event_queue: &'a mut PlatformEventQueue,
    pub app_event_queue: &'a mut AppEventQueue,
    pub renderer: &'a mut dyn Renderer,
    pub size: &'a mut (f64, f64),
    pub(crate) need_paint: &'a Cell<bool>,
    /// Engine-owned async executor. Handlers call `spawn_detached(...)` to run
    /// Rust futures (e.g. `clipboard.write_text`); the executor is driven each
    /// frame inside `flush`. See [`AsyncExecutor::spawn_detached`].
    pub async_executor: &'a Rc<AsyncExecutor>,
    /// Capability registry view. Handlers look up plugin-injected backends
    /// (e.g. `Clipboard`, `Http`) at dispatch time via
    /// `cx.capabilities.of::<C>()`. Missing capabilities return `None` —
    /// handlers must handle absence gracefully (typically silent drop with a
    /// `tracing::warn!`).
    pub capabilities: &'a Capabilities,
}

impl<'a> HandlerContext<'a> {
    pub fn request_paint(&self) {
        self.need_paint.set(true);
    }
}
