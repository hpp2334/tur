use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use boa_engine::context::time::Clock;
use parley::LayoutContext as ParleyLayoutContext;
use tur_shared::Constraints;

use crate::core::edgy_event::PendingMutationInvocationQueue;
use crate::core::elements::NodeTree;
use crate::core::event::queue::{AppEventQueue, PlatformEventQueue};
use crate::core::event::{PlatformEvent, PointerDeviceKind, PointerInput};
use crate::core::focus::FocusManager;
use crate::core::fonts::FontManager;
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::render::Renderer;
use crate::core::resource::ResourceMap;
use crate::core::shell::Shell;

pub struct TurAppContext {
    pub(crate) element_tree: NodeTree,
    pub(crate) mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub(crate) focus_manager: Rc<RefCell<FocusManager>>,
    pub(crate) resource_map: Rc<RefCell<ResourceMap>>,
    pub(crate) renderer: Box<dyn Renderer>,
    pub(crate) font_manager: FontManager,
    pub(crate) text_layout_cx: ParleyLayoutContext<[u8; 4]>,
    pub(crate) size: (f64, f64),
    pub(crate) platform_event_queue: PlatformEventQueue,
    pub(crate) app_event_queue: AppEventQueue,
    pub(crate) handlers: Vec<Box<dyn AppHandler>>,
    /// Shell layer: clock, pointer position, and cursor output (pushed to the
    /// embedder via a callback installed by a plugin). Owns the time source
    /// shared with the boa `Context`. See [`Shell`].
    pub(crate) shell: Shell,
}

impl fmt::Debug for TurAppContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurAppContext")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl TurAppContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        element_tree: NodeTree,
        mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
        focus_manager: Rc<RefCell<FocusManager>>,
        resource_map: Rc<RefCell<ResourceMap>>,
        renderer: Box<dyn Renderer>,
        font_loader: Box<dyn crate::core::fonts::FontLoader>,
        clock: Rc<dyn Clock>,
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
            platform_event_queue: PlatformEventQueue::new(),
            app_event_queue: AppEventQueue::new(),
            handlers: vec![],
            shell: Shell::new(clock),
        }
    }

    pub fn register_handler(&mut self, handler: Box<dyn AppHandler>) {
        self.handlers.push(handler);
    }

    /// Dispatch a platform (input) event to every registered handler via
    /// [`AppHandler::handle_platform_event`]. Mouse `PointerMove`s also
    /// update the shell's tracked pointer position and request a draw, since
    /// the cursor is resolved during paint (not in a handler).
    pub fn dispatch_platform_handlers(&mut self, event: &PlatformEvent, needs_draw: &Cell<bool>) {
        if let PlatformEvent::Pointer(PointerInput::PointerMove {
            position,
            device: PointerDeviceKind::Mouse,
        }) = event
        {
            self.shell.set_pointer_position(Some(*position));
            needs_draw.set(true);
        }

        let mut tree = self.element_tree.borrow_mut();
        let mut focus = self.focus_manager.borrow_mut();
        let mut mq = self.mutation_queue.borrow_mut();
        let mut cx = HandlerContext {
            element_tree: &mut tree,
            focus_manager: &mut focus,
            mutation_queue: &mut mq,
            platform_event_queue: &mut self.platform_event_queue,
            app_event_queue: &mut self.app_event_queue,
            renderer: self.renderer.as_mut(),
            size: &mut self.size,
            needs_draw,
        };
        for handler in &mut self.handlers {
            handler.handle_platform_event(&mut cx, event);
        }
    }

    /// Dispatch an engine-internal event to every registered handler via
    /// [`AppHandler::handle_app_event`].
    pub fn dispatch_app_handlers(&mut self, event: &crate::core::event::AppEvent, needs_draw: &Cell<bool>) {
        let mut tree = self.element_tree.borrow_mut();
        let mut focus = self.focus_manager.borrow_mut();
        let mut mq = self.mutation_queue.borrow_mut();
        let mut cx = HandlerContext {
            element_tree: &mut tree,
            focus_manager: &mut focus,
            mutation_queue: &mut mq,
            platform_event_queue: &mut self.platform_event_queue,
            app_event_queue: &mut self.app_event_queue,
            renderer: self.renderer.as_mut(),
            size: &mut self.size,
            needs_draw,
        };
        for handler in &mut self.handlers {
            handler.handle_app_event(&mut cx, event);
        }
    }

    pub fn layout(&mut self, dirty: Rc<Cell<bool>>, boa: &mut boa_engine::Context) {
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
            self.element_tree.clone(),
            self.mutation_queue.clone(),
            dirty,
            boa,
        );
    }

    pub fn render(&mut self) {
        let focused_node_id = self.focus_manager.borrow().focused();
        let resource_map = self.resource_map.borrow();
        let tree = self.element_tree.borrow();
        // Borrow the biz face for the paint pass, then flush the accumulated
        // cursor claims through the host API. The face is scoped so the
        // immutable shell borrow ends before `apply_changes` takes &mut.
        {
            // Clear the redraw deadline so only requests made during THIS
            // paint pass survive (elements repopulate it as they paint).
            self.shell.clear_redraw_deadline();
            let shell = self.shell.paint_face();
            self.renderer
                .render(&tree, focused_node_id, &resource_map, shell);
        }
        self.shell.apply_changes();
    }

    pub fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        self.renderer.render_to_pixels()
    }
}
