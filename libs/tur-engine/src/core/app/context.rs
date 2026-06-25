use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use parley::LayoutContext as ParleyLayoutContext;
use tur_shared::Constraints;

use crate::core::edgy_event::PendingMutationInvocationQueue;
use crate::core::elements::ElementTree;
use crate::core::event::queue::AppEventQueue;
use crate::core::event::AppEvent;
use crate::core::focus::FocusManager;
use crate::core::fonts::FontManager;
use crate::core::gesture::GestureEventComposer;
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::render::Renderer;
use crate::core::resource::ResourceMap;

pub struct TurAppContext {
    pub(crate) element_tree: Rc<RefCell<ElementTree>>,
    pub(crate) mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub(crate) focus_manager: Rc<RefCell<FocusManager>>,
    pub(crate) resource_map: Rc<RefCell<ResourceMap>>,
    pub(crate) renderer: Box<dyn Renderer>,
    pub(crate) font_manager: FontManager,
    pub(crate) text_layout_cx: ParleyLayoutContext<[u8; 4]>,
    pub(crate) size: (f64, f64),
    pub(crate) gesture_composer: GestureEventComposer,
    pub(crate) event_queue: AppEventQueue,
    pub(crate) handlers: Vec<Box<dyn AppHandler>>,
    /// The most recent cursor name set by a handler (e.g. "col-resize").
    /// Embedders poll this each frame to update the host canvas cursor.
    pub(crate) current_cursor: Rc<RefCell<Option<String>>>,
    /// Text written to the clipboard via `AppEvent::ClipboardWrite` since the
    /// last poll. `ClipboardWriteHandler` pushes here; embedders drain via
    /// `TurApp::take_clipboard_write()` once per frame.
    pub(crate) pending_clipboard_write: Rc<RefCell<Option<String>>>,
}

impl fmt::Debug for TurAppContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurAppContext")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl TurAppContext {
    pub fn new(
        element_tree: Rc<RefCell<ElementTree>>,
        mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
        focus_manager: Rc<RefCell<FocusManager>>,
        resource_map: Rc<RefCell<ResourceMap>>,
        renderer: Box<dyn Renderer>,
        font_loader: Box<dyn crate::core::fonts::FontLoader>,
    ) -> Self {
        let font_manager = FontManager::new(font_loader);
        Self {
            element_tree,
            mutation_queue,
            focus_manager,
            resource_map,
            renderer,
            font_manager,
            text_layout_cx: ParleyLayoutContext::new(),
            size: (400.0, 600.0),
            gesture_composer: GestureEventComposer::new(),
            event_queue: AppEventQueue::new(),
            handlers: vec![],
            current_cursor: Rc::new(RefCell::new(None)),
            pending_clipboard_write: Rc::new(RefCell::new(None)),
        }
    }

    pub fn register_handler(&mut self, handler: Box<dyn AppHandler>) {
        self.handlers.push(handler);
    }

    pub fn dispatch_handlers(&mut self, event: &AppEvent, needs_draw: &Cell<bool>) {
        let mut tree = self.element_tree.borrow_mut();
        let mut focus = self.focus_manager.borrow_mut();
        let mut mq = self.mutation_queue.borrow_mut();
        let mut cx = HandlerContext {
            element_tree: &mut tree,
            focus_manager: &mut focus,
            mutation_queue: &mut mq,
            event_queue: &mut self.event_queue,
            gesture_composer: &mut self.gesture_composer,
            renderer: self.renderer.as_mut(),
            size: &mut self.size,
            needs_draw,
            current_cursor: self.current_cursor.clone(),
            pending_clipboard_write: self.pending_clipboard_write.clone(),
        };
        for handler in &mut self.handlers {
            handler.handle_event(&mut cx, event);
        }
    }

    pub fn layout(&mut self, boa: &mut boa_engine::Context) {
        let (width, height) = self.size;
        let constraints = Constraints {
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        };

        let resource_map = self.resource_map.borrow();
        let mut tree = self.element_tree.borrow_mut();
        tree.compute_layout(
            &constraints,
            &mut self.font_manager,
            &mut self.text_layout_cx,
            &resource_map,
            boa,
        );
    }

    pub fn render(&mut self, now_ms: u64) {
        let focused_node_id = self.focus_manager.borrow().focused();
        let resource_map = self.resource_map.borrow();
        let tree = self.element_tree.borrow();
        self.renderer.render(&tree, focused_node_id, &resource_map, now_ms);
    }

    pub fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        self.renderer.render_to_pixels()
    }
}
