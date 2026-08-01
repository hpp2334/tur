use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::core::layout::Constraints;
use boa_engine::context::time::Clock;
use parley::LayoutContext as ParleyLayoutContext;

use crate::core::app::{AppEvent, AppEventQueue};
use crate::core::async_::AsyncExecutor;
use crate::core::capability::Capabilities;
use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::edgy::reactive::Store;
use crate::core::elements::NodeTree;
use crate::core::focus::FocusManager;
use crate::core::fonts::FontManager;
use crate::core::image_resource::ImageResourceMap;
use crate::core::platform::{PlatformEvent, PlatformEventQueue, PointerDeviceKind, PointerInput};
use crate::core::render::Renderer;
use crate::core::screen::Screen;
use crate::core::shell::Shell;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

pub struct TurAppContext {
    pub(crate) element_tree: NodeTree,
    pub(crate) mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub(crate) focus_manager: Rc<RefCell<FocusManager>>,
    pub(crate) image_resource_map: Rc<RefCell<ImageResourceMap>>,
    pub(crate) renderer: Box<dyn Renderer>,
    pub(crate) font_manager: FontManager,
    pub(crate) text_layout_cx: ParleyLayoutContext<[u8; 4]>,
    pub(crate) screen: Screen,
    pub(crate) platform_event_queue: PlatformEventQueue,
    pub(crate) app_event_queue: AppEventQueue,
    /// Engine-owned async executor. Cloned from the one `TurAppInternal`
    /// owns; surfaced to subsystems via [`SubsystemFlushContext`] so they
    /// can spawn Rust futures (clipboard writes, etc.) at dispatch time.
    pub(crate) async_executor: Rc<AsyncExecutor>,
    /// Capability registry view, shared with `TurJsContext.capabilities`.
    /// Surfaced to subsystems via [`SubsystemFlushContext::capabilities`] so
    /// they can look up backends (`Clipboard`, `Http`, etc.) at dispatch
    /// time.
    pub(crate) capabilities: Capabilities,
    /// Shell layer: clock, pointer position, and cursor output (pushed to the
    /// embedder via a callback installed by a plugin). Owns the time source
    /// shared with the boa `Context`. See [`Shell`].
    pub(crate) shell: Shell,
}

impl fmt::Debug for TurAppContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurAppContext")
            .field("logical_size", &self.screen.logical_size)
            .finish_non_exhaustive()
    }
}

impl TurAppContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        element_tree: NodeTree,
        mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
        focus_manager: Rc<RefCell<FocusManager>>,
        image_resource_map: Rc<RefCell<ImageResourceMap>>,
        renderer: Box<dyn Renderer>,
        font_context: crate::core::fonts::FontContext,
        font_loader: Rc<dyn crate::core::fonts::FontLoader>,
        async_executor: Rc<AsyncExecutor>,
        capabilities: Capabilities,
        clock: Rc<dyn Clock>,
        store: Store,
    ) -> Self {
        let font_manager = FontManager::from_context(font_context, font_loader);
        Self {
            element_tree,
            mutation_queue,
            focus_manager,
            image_resource_map,
            renderer,
            font_manager,
            text_layout_cx: ParleyLayoutContext::new(),
            screen: Screen::new(store),
            platform_event_queue: PlatformEventQueue::new(),
            app_event_queue: AppEventQueue::new(),
            async_executor,
            capabilities,
            shell: Shell::new(clock),
        }
    }

    /// Dispatch a platform (input) event to every registered subsystem via
    /// [`Subsystem::handle_platform_event`]. Mouse `PointerMove`s also
    /// update the shell's tracked pointer position and request a paint, since
    /// the cursor is resolved during paint (not in a subsystem).
    pub fn dispatch_platform_event(
        &mut self,
        boa: &mut boa_engine::Context,
        event: &PlatformEvent,
        need_paint: &Cell<bool>,
        subsystems: &mut [Box<dyn Subsystem>],
        signals: &crate::core::subsystem::FlushSignals<'_>,
    ) {
        if let PlatformEvent::Pointer(PointerInput::PointerMove {
            position,
            device: PointerDeviceKind::Mouse,
            time_ms: _,
        }) = event
        {
            self.shell.set_pointer_position(Some(*position));
            need_paint.set(true);
        }

        let mut cx = SubsystemFlushContext {
            boa,
            element_tree: self.element_tree.clone(),
            focus_manager: self.focus_manager.clone(),
            mutation_queue: self.mutation_queue.clone(),
            platform_event_queue: &mut self.platform_event_queue,
            app_event_queue: &mut self.app_event_queue,
            renderer: self.renderer.as_mut(),
            screen: &mut self.screen,
            need_paint,
            async_executor: &self.async_executor,
            capabilities: &self.capabilities,
            frame_id: signals.frame_id,
            sub_dirty: signals.sub_dirty,
            sub_request_frame: signals.sub_request_frame,
        };
        for sub in subsystems {
            sub.handle_platform_event(&mut cx, event);
        }
    }

    /// Dispatch an engine-internal event to every registered subsystem via
    /// [`Subsystem::handle_app_event`].
    pub fn dispatch_app_event(
        &mut self,
        boa: &mut boa_engine::Context,
        event: &AppEvent,
        need_paint: &Cell<bool>,
        subsystems: &mut [Box<dyn Subsystem>],
        signals: &crate::core::subsystem::FlushSignals<'_>,
    ) {
        let mut cx = SubsystemFlushContext {
            boa,
            element_tree: self.element_tree.clone(),
            focus_manager: self.focus_manager.clone(),
            mutation_queue: self.mutation_queue.clone(),
            platform_event_queue: &mut self.platform_event_queue,
            app_event_queue: &mut self.app_event_queue,
            renderer: self.renderer.as_mut(),
            screen: &mut self.screen,
            need_paint,
            async_executor: &self.async_executor,
            capabilities: &self.capabilities,
            frame_id: signals.frame_id,
            sub_dirty: signals.sub_dirty,
            sub_request_frame: signals.sub_request_frame,
        };
        for sub in subsystems {
            sub.handle_app_event(&mut cx, event);
        }
    }

    pub fn layout(&mut self, dirty: Rc<Cell<bool>>, boa: &mut boa_engine::Context) {
        let (width, height) = self.screen.logical_size;
        let constraints = Constraints {
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        };

        let image_resource_map = self.image_resource_map.borrow();
        let mut tree = self.element_tree.borrow_mut();
        tree.compute_layout(
            &constraints,
            &mut self.font_manager,
            &mut self.text_layout_cx,
            &image_resource_map,
            self.element_tree.clone(),
            self.mutation_queue.clone(),
            dirty,
            boa,
        );
    }

    pub fn render(&mut self) {
        let focused_node_id = self.focus_manager.borrow().focused();
        let image_resource_map = self.image_resource_map.borrow();
        let tree = self.element_tree.borrow();
        // Borrow the biz face for the paint pass, then flush the accumulated
        // cursor claims through the host API. The face is scoped so the
        // immutable shell borrow ends before `apply_changes` takes &mut.
        {
            let shell = self.shell.paint_face();
            self.renderer
                .render(&tree, focused_node_id, &image_resource_map, shell);
        }
        self.shell.apply_changes();
    }

    pub fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        self.renderer.render_to_pixels()
    }
}
