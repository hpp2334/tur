use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::core::layout::Constraints;
use boa_engine::context::time::Clock;
use parley::LayoutContext as ParleyLayoutContext;

use crate::core::app::{AppEvent, AppEventQueue};
use crate::core::async_::CompletionHandle;
use crate::core::capability::Capabilities;
use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::edgy::reactive::Store;
use crate::core::elements::NodeTree;
use crate::core::focus::FocusManager;
use crate::core::fonts::FontManager;
use crate::core::image_resource::ImageMetadataMap;
use crate::core::platform::{PlatformEvent, PlatformEventQueue, PointerDeviceKind, PointerInput};
use crate::core::render::{RecordingCanvas, RenderCommand};
use crate::core::scheduler::WorkerScheduler;
use crate::core::screen::Screen;
use crate::core::shell::Shell;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

pub struct TurAppContext {
    pub(crate) element_tree: NodeTree,
    pub(crate) mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub(crate) focus_manager: Rc<RefCell<FocusManager>>,
    /// Worker-side image metadata (natural sizes only — the pixel `Blob`
    /// lives on main, staged via `TurJsContext::pending_image_ships`).
    pub(crate) image_metadata_map: Rc<RefCell<ImageMetadataMap>>,
    pub(crate) font_manager: FontManager,
    pub(crate) text_layout_cx: ParleyLayoutContext<[u8; 4]>,
    pub(crate) screen: Screen,
    pub(crate) platform_event_queue: PlatformEventQueue,
    pub(crate) app_event_queue: AppEventQueue,
    /// Worker-thread scheduler — cloned from `TurAppInternal::worker_sched`.
    /// Surfaced to subsystems via [`SubsystemFlushContext`] so they can
    /// spawn Rust futures (clipboard writes, etc.) at dispatch time.
    pub(crate) worker_sched: WorkerScheduler,
    /// Cheap-cloned completion handle — cloned from
    /// `TurAppInternal::completion_handle`. Surfaced to subsystems so
    /// spawned futures can push promise-settle closures for `flush()` to
    /// drain under `&mut Context`.
    pub(crate) completion_handle: CompletionHandle,
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
        image_metadata_map: Rc<RefCell<ImageMetadataMap>>,
        font_context: crate::core::fonts::FontContext,
        font_loader: std::sync::Arc<dyn crate::core::fonts::FontLoader>,
        worker_sched: WorkerScheduler,
        completion_handle: CompletionHandle,
        capabilities: Capabilities,
        clock: Rc<dyn Clock>,
        store: Store,
    ) -> Self {
        let font_manager = FontManager::from_context(font_context, font_loader);
        Self {
            element_tree,
            mutation_queue,
            focus_manager,
            image_metadata_map,
            font_manager,
            text_layout_cx: ParleyLayoutContext::new(),
            screen: Screen::new(store),
            platform_event_queue: PlatformEventQueue::new(),
            app_event_queue: AppEventQueue::new(),
            worker_sched,
            completion_handle,
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
            screen: &mut self.screen,
            need_paint,
            worker_sched: &self.worker_sched,
            completion_handle: &self.completion_handle,
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
            screen: &mut self.screen,
            need_paint,
            worker_sched: &self.worker_sched,
            completion_handle: &self.completion_handle,
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

        let image_metadata_map = self.image_metadata_map.borrow();
        let mut tree = self.element_tree.borrow_mut();
        tree.compute_layout(
            &constraints,
            &mut self.font_manager,
            &mut self.text_layout_cx,
            &image_metadata_map,
            self.element_tree.clone(),
            self.mutation_queue.clone(),
            dirty,
            boa,
        );
    }

    /// Walk the element tree with a [`RecordingCanvas`] to capture per-node
    /// paint ops + boundaries, post-process the recording into
    /// `Vec<RenderCommand>` (paint commands in playback order), and return
    /// the batch.
    ///
    /// The caller is responsible for shipping the batch to whichever
    /// thread/realm owns the actual renderer. The worker stores it in
    /// `TurAppInternal::pending_render_batch` for `MainBackend::worker_loop`
    /// to drain and ship via `MainMsg::RenderCommands`.
    pub fn build_render_batch(&mut self) -> Vec<RenderCommand> {
        let focused_node_id = self.focus_manager.borrow().focused();

        // Record the paint pass. Seed the recording canvas with the logical
        // viewport as the bottom-of-stack clip so off-screen subtrees are
        // culled during the walk (content outside the screen is invisible
        // anyway). Explicit element clips (ScrollView, overflow-Flex, …)
        // push further inner clips intersected with this viewport.
        let tree = self.element_tree.borrow();
        let (vp_w, vp_h) = self.screen.logical_size;
        let mut recording = RecordingCanvas::new_with_viewport(vello_common::kurbo::Rect::new(
            0.0, 0.0, vp_w, vp_h,
        ));
        {
            let shell = self.shell.paint_face();
            tree.paint(
                &mut recording,
                focused_node_id,
                &self.image_metadata_map.borrow(),
                shell,
            );
        }
        drop(tree);

        // Collect the paint commands into one batch.
        let batch = recording.into_render_commands();

        // Flush cursor claims accumulated during the record pass.
        self.shell.apply_changes();

        batch
    }
}
