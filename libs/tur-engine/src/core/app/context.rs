use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::core::element::ViewRootId;
use crate::core::layout::Constraints;
use boa_engine::context::time::Clock;
use parley::LayoutContext as ParleyLayoutContext;

use crate::core::app::view_roots::SharedViewRoots;
use crate::core::app::{AppEvent, AppEventQueue};
use crate::core::async_::CompletionHandle;
use crate::core::capability::Capabilities;
use crate::core::edgy::mutation::PendingMutationInvocationQueue;
use crate::core::edgy::reactive::Store;
use crate::core::focus::FocusManager;
use crate::core::fonts::FontManager;
use crate::core::image_resource::ImageManager;
use crate::core::platform::{PlatformEvent, PlatformEventQueue};
use crate::core::render::{RecordingCanvas, RenderCommand};
use crate::core::scheduler::WorkerScheduler;
use crate::core::shell::Shell;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

pub struct TurAppContext {
    /// The instance's view-root registry — one element tree + one screen per
    /// view root (see `core::app::view_roots`). Shared with
    /// [`TurInstanceContext`](crate::core::js_runtime::TurInstanceContext)
    /// and subsystems via [`SubsystemFlushContext`].
    pub(crate) view_roots: SharedViewRoots,
    pub(crate) mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
    pub(crate) focus_manager: Rc<RefCell<FocusManager>>,
    /// Worker-side image state (natural-size map + next-id counter — the
    /// pixel `Blob` lives on main, shipped via `MainMsg::UploadImage`).
    pub(crate) image_manager: Rc<RefCell<ImageManager>>,
    pub(crate) font_manager: FontManager,
    pub(crate) text_layout_cx: ParleyLayoutContext<[u8; 4]>,
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
    /// Capability registry view, shared with `TurInstanceContext.capabilities`.
    /// Surfaced to subsystems via [`SubsystemFlushContext`] so they can
    /// look up backends (`Clipboard`, `Http`, etc.) at dispatch
    /// time.
    pub(crate) capabilities: Capabilities,
    /// Shell layer: clock, pointer position, and cursor output (pushed to the
    /// embedder via a callback installed by a plugin). Owns the time source
    /// shared with the boa `Context`. See [`Shell`].
    pub(crate) shell: Shell,
}

impl fmt::Debug for TurAppContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let roots = self.view_roots.borrow();
        f.debug_struct("TurAppContext")
            .field(
                "view_roots",
                &roots
                    .slots()
                    .iter()
                    .map(|s| (s.name.clone(), s.screen.logical_size))
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl TurAppContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        view_roots: SharedViewRoots,
        mutation_queue: Rc<RefCell<PendingMutationInvocationQueue>>,
        focus_manager: Rc<RefCell<FocusManager>>,
        image_manager: Rc<RefCell<ImageManager>>,
        font_context: crate::core::fonts::FontContext,
        font_loader: std::sync::Arc<dyn crate::core::fonts::FontLoader>,
        worker_sched: WorkerScheduler,
        completion_handle: CompletionHandle,
        capabilities: Capabilities,
        clock: Rc<dyn Clock>,
        store: Store,
    ) -> Self {
        let _ = store;
        let font_manager = FontManager::from_context(font_context, font_loader);
        Self {
            view_roots,
            mutation_queue,
            focus_manager,
            image_manager,
            font_manager,
            text_layout_cx: ParleyLayoutContext::new(),
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
        if let crate::core::platform::ShellEventPayload::Pointer {
            input:
                crate::core::platform::PointerInput::PointerMove {
                    position,
                    device: crate::core::platform::PointerDeviceKind::Mouse,
                    time_ms: _,
                },
        } = event.payload()
        {
            self.shell
                .set_pointer_position(event.view_root_id(), *position);
            need_paint.set(true);
        }

        let mut cx = SubsystemFlushContext {
            boa,
            view_roots: self.view_roots.clone(),
            focus_manager: self.focus_manager.clone(),
            mutation_queue: self.mutation_queue.clone(),
            platform_event_queue: &mut self.platform_event_queue,
            app_event_queue: &mut self.app_event_queue,
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
            view_roots: self.view_roots.clone(),
            focus_manager: self.focus_manager.clone(),
            mutation_queue: self.mutation_queue.clone(),
            platform_event_queue: &mut self.platform_event_queue,
            app_event_queue: &mut self.app_event_queue,
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

    /// Lay out every **setup** view root's tree. Each root's constraints
    /// come from its own screen (`min == max == root viewport`), so roots
    /// are laid out independently — a resize of one root never re-lays-out
    /// another.
    pub fn layout(&mut self, dirty: Rc<Cell<bool>>, boa: &mut boa_engine::Context) {
        let image_manager = self.image_manager.borrow();
        let setup_roots = self.view_roots.borrow().setup_roots();
        for (root_id, tree) in setup_roots {
            let (width, height) = self
                .view_roots
                .borrow()
                .get(root_id)
                .map(|s| s.screen.logical_size)
                .unwrap_or((0.0, 0.0));
            let constraints = Constraints {
                min_width: width,
                max_width: width,
                min_height: height,
                max_height: height,
            };
            tree.compute_layout(
                &constraints,
                &mut self.font_manager,
                &mut self.text_layout_cx,
                &image_manager,
                tree.clone(),
                self.mutation_queue.clone(),
                dirty.clone(),
                boa,
            );
        }
    }

    /// Record one paint batch **per setup view root** (each seeded with that
    /// root's logical viewport as the bottom-of-stack clip so off-screen
    /// subtrees are culled during the walk). The caller ships the batches to
    /// main tagged with the root id (`MainMsg::RenderCommands { root, .. }`).
    pub fn build_render_batches(&mut self) -> Vec<(ViewRootId, Vec<RenderCommand>)> {
        let focused_node_id = self.focus_manager.borrow().focused();
        let image_manager = self.image_manager.borrow();
        let setup_roots = self.view_roots.borrow().setup_roots();

        let mut batches = Vec::with_capacity(setup_roots.len());
        for (root_id, tree) in setup_roots {
            let viewport = self
                .view_roots
                .borrow()
                .get(root_id)
                .map(|s| s.viewport_rect())
                .unwrap_or_else(|| vello_common::kurbo::Rect::ZERO);
            let mut recording = RecordingCanvas::new_with_viewport(viewport);
            {
                let shell = self.shell.paint_face_for(root_id);
                tree.paint(&mut recording, focused_node_id, &image_manager, shell);
            }
            batches.push((root_id, recording.into_render_commands()));
        }

        // Flush cursor claims accumulated during the record pass.
        self.shell.apply_changes();

        batches
    }
}
