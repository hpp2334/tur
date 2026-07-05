use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use boa_engine::context::time::FixedClock;
use parley::LayoutContext as ParleyLayoutContext;
use tur_shared::Constraints;

use crate::core::edgy_event::PendingMutationInvocationQueue;
use crate::core::elements::NodeTree;
use crate::core::event::queue::AppEventQueue;
use crate::core::event::{AppEvent, AppGestureEvent};
use crate::core::focus::FocusManager;
use crate::core::fonts::FontManager;
use crate::core::gesture::GestureEventComposer;
use crate::core::handler::{AppHandler, HandlerContext};
use crate::core::render::Renderer;
use crate::core::resource::ResourceMap;
use crate::core::shell::Shell;

pub struct TurAppContext {
    pub element_tree: NodeTree,
    pub mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub focus_manager: Rc<RefCell<FocusManager>>,
    pub resource_map: Rc<RefCell<ResourceMap>>,
    pub renderer: Box<dyn Renderer>,
    pub font_manager: FontManager,
    pub text_layout_cx: ParleyLayoutContext<[u8; 4]>,
    pub size: (f64, f64),
    pub gesture_composer: GestureEventComposer,
    pub event_queue: AppEventQueue,
    pub handlers: Vec<Box<dyn AppHandler>>,
    /// Shell layer: clock, pointer position, and cursor output (pushed to the
    /// embedder via a callback installed by a plugin). Owns the time source
    /// shared with the boa `Context`. See [`Shell`].
    pub shell: Shell,
    /// Text written to the clipboard via `AppEvent::ClipboardWrite` since the
    /// last poll. `ClipboardWriteHandler` pushes here; embedders drain via
    /// `TurApp::take_clipboard_write()` once per frame.
    pub pending_clipboard_write: Rc<RefCell<Option<String>>>,
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
        clock: Rc<FixedClock>,
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
            shell: Shell::new(clock),
            pending_clipboard_write: Rc::new(RefCell::new(None)),
        }
    }

    pub fn register_handler(&mut self, handler: Box<dyn AppHandler>) {
        self.handlers.push(handler);
    }

    pub fn dispatch_handlers(&mut self, event: &AppEvent, needs_draw: &Cell<bool>) {
        // Track the last pointer position so the paint pass can hit-test
        // MouseRegions for cursor resolution. A move must trigger a render
        // because the cursor is now computed during paint (not in a handler).
        if let AppEvent::Gesture(AppGestureEvent::PointerMove { position }) = event {
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
            event_queue: &mut self.event_queue,
            gesture_composer: &mut self.gesture_composer,
            renderer: self.renderer.as_mut(),
            size: &mut self.size,
            needs_draw,
            pending_clipboard_write: self.pending_clipboard_write.clone(),
        };
        for handler in &mut self.handlers {
            handler.handle_event(&mut cx, event);
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
