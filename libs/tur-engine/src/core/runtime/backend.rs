//! Backend abstraction for [`TurApp`](crate::TurApp) — encapsulates **where
//! the engine state lives** and **how methods dispatch**.
//!
//! - [`InlineBackend`] runs everything on the calling thread (today's
//!   behavior). Used by `TurRuntime::create_app` / `create_headless_app`
//!   and by every test.
//! - `ThreadedBackend` (Phase 7 follow-up) runs the engine on a worker
//!   thread, dispatching via `mpsc` channels. Used by
//!   `TurRuntime::create_app_threaded` (production).
//!
//! `TurApp` holds `Box<dyn TurAppBackend>` + the main-side scheduling state
//! (driver / wake_fn / after_frame hook). Public methods delegate to the
//! backend. The two backends are interchangeable from the embedder's
//! perspective — the same `TurApp` API works either way.
//!
//! ## Escape hatches (with_boa_context / with_element)
//!
//! These are generic over the return type `R`, which can't be expressed in
//! a trait object directly. The trait exposes type-erased `_dyn` variants
//! taking `Box<dyn FnOnce(...) -> Box<dyn Any + Send>>`; `TurApp` wraps
//! them in ergonomic generic helpers that box/unbox automatically. The
//! closures must be `Send + 'static` so threaded mode can ship them across
//! the thread boundary (inline mode doesn't actually need `Send`, but the
//! uniform API simplifies the trait).

use std::any::Any;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use boa_engine::{Context, Source};

use crate::FocusedState;
use crate::core::app::AppEvent;
use crate::core::app::{FrameOutcome, ModuleError, TurAppInternal, WorkerMsg};
use crate::core::async_::TurJobExecutor;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, DevNodeData};
use crate::core::event_bus::EventBus;
use crate::core::platform::CursorBackend;
use crate::error::TurError;

/// Type-erased closure for [`TurAppBackend::with_boa_context_dyn`].
///
/// **Not `Send`** — inline-only. The threaded backend will panic if this
/// is called (it doesn't support arbitrary closure-based boa access; it
/// uses specific `WorkerMsg` RPC variants instead). Production threaded
/// code doesn't need the escape hatches; they're for tests and embedder
/// debugging.
pub type BoaClosure = Box<dyn FnOnce(&mut Context) -> Box<dyn Any>>;

/// Type-erased closure for [`TurAppBackend::with_element_dyn`].
pub type ElementClosure = Box<dyn FnOnce(&AnyElement) -> Box<dyn Any>>;

/// Type-erased return value from the escape hatches.
pub type AnySend = Box<dyn Any>;

/// Backend abstraction — see [module docs](self) for the inline vs threaded
/// split. All methods are synchronous from the caller's perspective;
/// `ThreadedBackend` blocks on its reply channel internally to preserve
/// the synchronous API.
pub trait TurAppBackend: 'static {
    /// Internal dispatch — every [`WorkerMsg`] variant is processed here.
    /// Inline runs on the calling thread; threaded sends to the worker.
    fn handle_worker_msg(&self, msg: WorkerMsg);

    /// Run one frame's flush + render. Returns the outcome including how
    /// the next frame should be scheduled.
    fn pump(&self) -> Result<FrameOutcome, TurError>;

    /// Push an engine-internal event onto the app-event bus. Inline writes
    /// directly to the queue; threaded must RPC (`AppEvent`'s `Custom`
    /// payload may be `!Send`, so threaded panics until Phase 7 wires the
    /// `Send` bound — see `WorkerMsg::AppEvent`).
    fn push_app_event(&self, event: AppEvent);

    /// Always-installed event bus.
    fn event_bus(&self) -> Rc<EventBus>;

    /// Combined focused-element state.
    fn focused_state(&self) -> FocusedState;
    fn focused_element(&self) -> Option<ElementNodeId>;
    fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)>;
    fn focused_is_editable(&self) -> bool;

    /// Path-based element lookup.
    fn query_element(&self, key: &[&str]) -> Option<NodeId>;

    /// Dev tooling.
    fn dev_tool_element_tree(&self) -> Option<DevNodeData>;
    fn dev_tool_get_element(&self, id: NodeId) -> Option<DevNodeData>;

    /// Read rendered pixels (used by screenshot tests).
    fn render_to_pixels(&self) -> Option<Vec<u8>>;

    /// Override the cursor backend. Per-instance (the wasm embedder's
    /// `WasmCursor` holds the canvas DOM element).
    fn set_cursor_backend(&self, backend: Rc<RefCell<dyn CursorBackend>>);

    /// Escape hatch — run a closure with the boa `Context`. **Inline-only**:
    /// the threaded backend panics (no mechanism to ship arbitrary
    /// closures across threads). Production threaded code uses specific
    /// `WorkerMsg` RPC variants; this escape hatch is for tests and
    /// embedder debugging.
    fn with_boa_context_dyn(&self, f: BoaClosure) -> AnySend;

    /// Escape hatch — run a closure with an element by id. Returns
    /// `None` if the element doesn't exist. **Inline-only** (see
    /// [`Self::with_boa_context_dyn`]).
    fn with_element_dyn(&self, id: ElementNodeId, f: ElementClosure) -> Option<AnySend>;

    /// Trace-mode handle for the live element tree.
    #[cfg(feature = "trace")]
    fn element_tree_handle(&self) -> std::cell::Ref<'_, crate::core::elements::NodeTreeData>;
}

// ---------------------------------------------------------------------------
// InlineBackend — today's behavior, engine state lives on the calling thread
// ---------------------------------------------------------------------------

pub struct InlineBackend {
    pub(crate) boa_context: RefCell<Context>,
    pub(crate) internal: TurAppInternal,
    pub(crate) executor: Rc<TurJobExecutor>,
}

impl InlineBackend {
    pub(crate) fn new(
        boa_context: Context,
        internal: TurAppInternal,
        executor: Rc<TurJobExecutor>,
    ) -> Self {
        Self {
            boa_context: RefCell::new(boa_context),
            internal,
            executor,
        }
    }

    fn load_js_inner(&self, source: &str) -> Result<(), ModuleError> {
        let mut boa = self.boa_context.borrow_mut();
        boa.eval(Source::from_bytes(source).with_path(Path::new("bundle.js")))
            .map_err(|e| {
                tracing::error!("JS eval error: {e}");
                ModuleError::Eval(e.to_string())
            })?;
        if let Err(e) = self.executor.drain(&mut boa) {
            tracing::error!("load_js drain error: {e}");
        }
        tracing::info!("load_js: bundle evaluated successfully");
        Ok(())
    }

    fn load_module_inner(&self, source: &str) -> Result<(), ModuleError> {
        let mut boa = self.boa_context.borrow_mut();
        let module = boa_engine::Module::parse(
            Source::from_bytes(source).with_path(Path::new("entry.mjs")),
            None,
            &mut boa,
        )
        .map_err(|e| {
            tracing::error!("module parse error: {e}");
            ModuleError::Parse(e.to_string())
        })?;
        let _promise = module.load_link_evaluate(&mut boa);
        if let Err(e) = boa.run_jobs() {
            tracing::error!("module run_jobs error: {e}");
        }
        drop(boa);
        if let Err(e) = self.executor.drain(&mut self.boa_context.borrow_mut()) {
            tracing::error!("load_module drain error: {e}");
        }
        Ok(())
    }

    fn eval_module_inner(&self, source: &str) -> Result<(), ModuleError> {
        let mut boa = self.boa_context.borrow_mut();
        let module = boa_engine::Module::parse(
            Source::from_bytes(source).with_path(Path::new("eval.mjs")),
            None,
            &mut boa,
        )
        .map_err(|e| {
            tracing::error!("eval_module parse error: {e}");
            ModuleError::Parse(e.to_string())
        })?;
        let _promise = module.load_link_evaluate(&mut boa);
        if let Err(e) = boa.run_jobs() {
            tracing::error!("eval_module run_jobs error: {e}");
        }
        drop(boa);
        let _ = self.executor.drain(&mut self.boa_context.borrow_mut());
        Ok(())
    }
}

impl TurAppBackend for InlineBackend {
    fn handle_worker_msg(&self, msg: WorkerMsg) {
        match msg {
            WorkerMsg::PlatformEvent(event) => {
                self.internal
                    .app_context
                    .borrow_mut()
                    .platform_event_queue
                    .push(event);
                // Driver re-arm happens on TurApp (main side) —
                // handle_worker_msg just queues.
            }
            WorkerMsg::RequestPaint => {
                self.internal.js_context.need_paint.set(true);
            }
            WorkerMsg::Wake => {
                // Single-threaded: caller (`pump`) drives flush directly so
                // it can return FrameOutcome. Threaded would run flush here.
            }
            WorkerMsg::LoadModule { source, reply } => {
                let res = self.load_module_inner(&source);
                reply.send(res);
            }
            WorkerMsg::LoadJs { source, reply } => {
                let res = self.load_js_inner(&source);
                reply.send(res);
            }
            WorkerMsg::EvalModule { source, reply } => {
                let res = self.eval_module_inner(&source);
                reply.send(res);
            }
            WorkerMsg::DevElementTree { reply } => {
                let snap = self.dev_tool_element_tree();
                reply.send(snap);
            }
            WorkerMsg::DevGetElement { id, reply } => {
                let snap = self.dev_tool_get_element(id);
                reply.send(snap);
            }
            WorkerMsg::EventBusToJs(_bytes) => {
                tracing::trace!(
                    "EventBusToJs: {} bytes (Phase 5 wires delivery)",
                    _bytes.len()
                );
            }
            WorkerMsg::Destroy { reply } => {
                reply.send(());
            }
        }
    }

    fn pump(&self) -> Result<FrameOutcome, TurError> {
        self.handle_worker_msg(WorkerMsg::Wake);
        let mut boa = self.boa_context.borrow_mut();
        self.internal.flush(&mut boa)
    }

    fn push_app_event(&self, event: AppEvent) {
        self.internal
            .app_context
            .borrow_mut()
            .app_event_queue
            .push(event);
    }

    fn event_bus(&self) -> Rc<EventBus> {
        self.internal.event_bus.clone()
    }

    fn focused_state(&self) -> FocusedState {
        FocusedState {
            is_editable: self.focused_is_editable(),
            cursor_rect: self.focused_cursor_rect(),
        }
    }

    fn focused_element(&self) -> Option<ElementNodeId> {
        self.internal.js_context.focus_manager.borrow().focused()
    }

    fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let focused_id = self.focused_element()?;
        let tree = self.internal.js_context.element_tree.borrow();

        let mut abs_x = 0.0f64;
        let mut abs_y = 0.0f64;
        let mut current: Option<NodeId> = Some(focused_id.into());
        while let Some(id) = current {
            let node = tree.get_element(ElementNodeId::new(id.as_u64()))?;
            abs_x += node.computed_layout.offset.x;
            abs_y += node.computed_layout.offset.y;
            current = node.parent;
        }

        let node = tree.get_element(focused_id)?;
        let element = node.element.as_ref()?;
        let (cx, cy, cw, ch) = element.cursor_rect_relative()?;

        Some((abs_x + cx, abs_y + cy, cw, ch))
    }

    fn focused_is_editable(&self) -> bool {
        use crate::core::focus::helper;
        let tree = self.internal.js_context.element_tree.borrow();
        let focus = self.internal.js_context.focus_manager.borrow();
        helper::focused_is_editable(&tree, &focus)
    }

    fn query_element(&self, key: &[&str]) -> Option<NodeId> {
        self.internal
            .js_context
            .element_tree
            .borrow()
            .query_element(key)
    }

    fn dev_tool_element_tree(&self) -> Option<DevNodeData> {
        let tree = self.internal.js_context.element_tree.borrow();
        let root_id = tree.root_element_id()?;
        tree.dev_tool_node(root_id.into())
    }

    fn dev_tool_get_element(&self, id: NodeId) -> Option<DevNodeData> {
        self.internal
            .js_context
            .element_tree
            .borrow()
            .dev_tool_node(id)
    }

    fn render_to_pixels(&self) -> Option<Vec<u8>> {
        self.internal.app_context.borrow_mut().render_to_pixels()
    }

    fn set_cursor_backend(&self, backend: Rc<RefCell<dyn CursorBackend>>) {
        self.internal
            .app_context
            .borrow_mut()
            .shell
            .set_cursor_platform(backend);
    }

    fn with_boa_context_dyn(&self, f: BoaClosure) -> AnySend {
        f(&mut self.boa_context.borrow_mut())
    }

    fn with_element_dyn(&self, id: ElementNodeId, f: ElementClosure) -> Option<AnySend> {
        let tree = self.internal.js_context.element_tree.borrow();
        let node = tree.get_element(id)?;
        let element = node.element.as_ref()?;
        Some(f(element))
    }

    #[cfg(feature = "trace")]
    fn element_tree_handle(&self) -> std::cell::Ref<'_, crate::core::elements::NodeTreeData> {
        self.internal.js_context.element_tree.borrow()
    }
}
