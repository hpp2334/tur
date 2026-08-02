//! Backend abstraction for [`TurApp`](crate::TurApp) — encapsulates **where
//! the engine state lives** and **how methods dispatch**.
//!
//! - [`InlineBackend`] runs everything on the calling thread (today's
//!   behavior). Used by `TurRuntime::create_app` / `create_headless_app`
//!   and by every test.
//! - [`ThreadedBackend`] runs the engine on a worker thread, dispatching
//!   via `mpsc` channels. Production use; some methods panic until
//!   Phase 8 wires cross-thread variants (see the trait impl docs).
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
//! taking [`BoaClosure`] / [`ElementClosure`]; `TurApp` wraps them in
//! ergonomic generic helpers that box/unbox automatically. **Inline-only**:
//! closures can't easily be made `Send` without restricting the inline API,
//! so threaded mode panics — production threaded code uses RPC variants
//! (`load_module`, `dev_tool_*`, etc.) instead.

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

    /// Cross-thread-safe event bus handle. Holds just the queue clones
    /// (Arc<Mutex>) — safe to send across threads. Inline returns a
    /// handle constructed from `internal.event_bus.queues()`; threaded
    /// returns a pre-stored handle (constructed at worker-spawn time).
    fn event_bus_handle(&self) -> crate::core::event_bus::EventBusHandle;

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

    /// Read the latest cursor applied during the last flush (or `None` if
    /// no pointer was over the surface / no cursor change happened).
    /// Used by `ThreadedBackend` to ship cursor changes via
    /// `MainMsg::CursorChanged`.
    pub(crate) fn last_applied_cursor(&self) -> Option<crate::core::platform::Cursor> {
        self.internal
            .app_context
            .borrow()
            .shell
            .last_applied_cursor()
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
            WorkerMsg::QueryFocusedState { reply } => {
                reply.send(self.focused_state());
            }
            WorkerMsg::QueryFocusedElement { reply } => {
                reply.send(self.focused_element());
            }
            WorkerMsg::QueryFocusedCursorRect { reply } => {
                reply.send(self.focused_cursor_rect());
            }
            WorkerMsg::QueryFocusedIsEditable { reply } => {
                reply.send(self.focused_is_editable());
            }
            WorkerMsg::QueryElement { key, reply } => {
                let key_refs: Vec<&str> = key.iter().map(|s| s.as_str()).collect();
                reply.send(self.query_element(&key_refs));
            }
            WorkerMsg::EventBusToJs(_bytes) => {
                tracing::trace!(
                    "EventBusToJs: {} bytes (Phase 5 wires delivery)",
                    _bytes.len()
                );
            }
            WorkerMsg::AppEvent(event) => {
                self.push_app_event(event);
            }
            WorkerMsg::RenderToPixels { reply } => {
                reply.send(self.render_to_pixels());
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

    fn event_bus_handle(&self) -> crate::core::event_bus::EventBusHandle {
        let (h, j) = self.internal.event_bus.queues();
        crate::core::event_bus::EventBusHandle::from_queues(h, j)
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

// ---------------------------------------------------------------------------
// ThreadedBackend — engine state on a worker thread, RPC via mpsc
// ---------------------------------------------------------------------------

use std::sync::Mutex;
use std::sync::mpsc;

use crate::core::app::MainMsg;

/// Worker thread owner of [`InlineBackend`]. The main thread holds
/// [`ThreadedBackend`] (just the channel endpoints) and dispatches via RPC.
///
/// ## What works cross-thread
///
/// - [`handle_worker_msg`](TurAppBackend::handle_worker_msg) for input
///   (`PlatformEvent`, `RequestPaint`, `EventBusToJs`) — fire-and-forget.
/// - [`pump`](TurAppBackend::pump) — sends `Wake`, blocks on
///   `MainMsg::FrameOutcome`.
/// - RPC methods with `Reply` slots (`LoadModule`, `LoadJs`, `EvalModule`,
///   `DevElementTree`, `DevGetElement`) — `TurApp` blocks on `Reply::recv`.
///
/// ## What panics (deferred to Phase 8)
///
/// - [`event_bus`](TurAppBackend::event_bus) — `EventBus` is `Rc`-backed.
///   Phase 8 will introduce a main-side proxy or migrate the bus to
///   `Arc<Mutex<>>`.
/// - [`push_app_event`](TurAppBackend::push_app_event) — `AppEvent`'s
///   `Custom` payload may be `!Send`. Phase 8 wires the bound + a
///   `WorkerMsg::AppEvent` variant.
/// - Escape hatches (`with_boa_context_dyn`, `with_element_dyn`) — closures
///   can't be made `Send` without restricting the inline API. Tests use
///   inline mode; production doesn't need these.
/// - Direct-read accessors (`focused_state`, `query_element`, etc.) — these
///   need either RPC variants or main-side caching from `MainMsg`. Phase 8
///   adds the variants; for now, use `dev_tool_*` which IS RPC-able.
pub struct ThreadedBackend {
    worker_tx: mpsc::Sender<WorkerMsg>,
    main_rx: Mutex<mpsc::Receiver<MainMsg>>,
    /// Main-side cursor backend. Worker emits `MainMsg::CursorChanged`
    /// on cursor state change; main applies it here during `pump`. Set
    /// via `set_cursor_backend` (called by embedder after
    /// `create_app_threaded`). Stored as `RefCell` since
    /// `ThreadedBackend` lives on the main thread only.
    cursor_backend: RefCell<Option<Rc<RefCell<dyn CursorBackend>>>>,
    /// Cross-thread event bus handle. Routes `emit_to_js` via
    /// `WorkerMsg::EventBusToJs` (channel mode — no shared queues with
    /// the worker, since the worker's `EventBus` is constructed inside
    /// `build_inline_backend` and isn't reachable from main).
    event_bus_handle: crate::core::event_bus::EventBusHandle,
}

impl ThreadedBackend {
    /// Spawn a worker thread that owns an [`InlineBackend`] produced by
    /// `backend_factory`. The factory runs on the worker thread (so it can
    /// construct `!Send` types like `Rc<dyn Clock>` and `boa::Context`).
    ///
    /// The factory must be `Send + 'static` — capture only `Send` config
    /// (plugin vecs, capability factories, etc.), not `Rc`/`RefCell` state.
    pub fn new(backend_factory: impl FnOnce() -> InlineBackend + Send + 'static) -> Self {
        let (worker_tx, worker_rx) = mpsc::channel::<WorkerMsg>();
        let (main_tx, main_rx) = mpsc::channel::<MainMsg>();

        std::thread::Builder::new()
            .name("tur-worker".into())
            .spawn(move || {
                let backend = backend_factory();
                worker_loop(backend, worker_rx, main_tx);
            })
            .expect("failed to spawn tur worker thread");

        Self {
            worker_tx: worker_tx.clone(),
            main_rx: Mutex::new(main_rx),
            cursor_backend: RefCell::new(None),
            event_bus_handle: crate::core::event_bus::EventBusHandle::from_channel(worker_tx),
        }
    }
}

fn worker_loop(
    backend: InlineBackend,
    worker_rx: mpsc::Receiver<WorkerMsg>,
    main_tx: mpsc::Sender<MainMsg>,
) {
    let mut last_cursor: Option<crate::core::platform::Cursor> = None;
    while let Ok(msg) = worker_rx.recv() {
        match msg {
            WorkerMsg::Wake => {
                let outcome = backend.pump();
                let payload = match outcome {
                    Ok(fo) => Ok(fo),
                    Err(e) => {
                        tracing::error!("worker pump error: {e}");
                        Err(e.to_string())
                    }
                };
                let _ = main_tx.send(MainMsg::FrameOutcome(payload));
                // Ship cursor changes (deduped against the last emitted).
                let current = backend.last_applied_cursor();
                if current != last_cursor {
                    last_cursor = current;
                    let _ = main_tx.send(MainMsg::CursorChanged(current.unwrap_or_default()));
                }
            }
            WorkerMsg::Destroy { reply } => {
                reply.send(());
                break;
            }
            // All other variants (PlatformEvent, RequestPaint, LoadModule,
            // Dev*, EventBusToJs) delegate to the inline dispatch — RPC
            // variants fire their own ReplySender, so main blocks on
            // `Reply::recv` independently of `main_tx`.
            other => backend.handle_worker_msg(other),
        }
    }
}

impl TurAppBackend for ThreadedBackend {
    fn handle_worker_msg(&self, msg: WorkerMsg) {
        // Fire-and-forget for non-RPC variants; RPC variants carry their
        // own `ReplySender` and the caller blocks on `Reply::recv`.
        let _ = self.worker_tx.send(msg);
    }

    fn pump(&self) -> Result<FrameOutcome, TurError> {
        self.worker_tx
            .send(WorkerMsg::Wake)
            .map_err(|_| TurError::Other("worker gone".into()))?;
        let main_rx = self.main_rx.lock().expect("main_rx poisoned");
        loop {
            match main_rx.recv() {
                Ok(MainMsg::FrameOutcome(result)) => return result.map_err(TurError::Other),
                Ok(MainMsg::CursorChanged(cursor)) => {
                    // Apply to the main-side cursor backend (set via
                    // `set_cursor_backend`). No backend → no-op.
                    if let Some(backend) = self.cursor_backend.borrow().as_ref() {
                        backend.borrow_mut().set_cursor(cursor);
                    }
                }
                // FocusedStateChanged / EventBusToHost / RenderCommands /
                // DevReply / Destroyed — pump ignores for now; Phase 8
                // routes them to embedder-side handlers.
                Ok(_) => continue,
                Err(_) => return Err(TurError::Other("worker gone".into())),
            }
        }
    }

    fn push_app_event(&self, event: AppEvent) {
        // AppEvent's Custom payload's Send + Sync bound (Phase 6 prep)
        // lets us ship it across the thread boundary. Fire-and-forget —
        // no Reply needed.
        let _ = self.worker_tx.send(WorkerMsg::AppEvent(event));
    }

    fn event_bus(&self) -> Rc<EventBus> {
        // The full EventBus lives on the worker. For threaded mode,
        // embedders use `event_bus_handle()` (the cross-thread-safe
        // handle that routes via mpsc). The inline-only `event_bus()`
        // panics here — production threaded code uses the handle.
        unimplemented!("event_bus (full API) not supported in threaded mode; use event_bus_handle()")
    }

    fn event_bus_handle(&self) -> crate::core::event_bus::EventBusHandle {
        self.event_bus_handle.clone()
    }

    fn focused_state(&self) -> FocusedState {
        let (tx, rx) = crate::core::app::Reply::<FocusedState>::pair();
        let _ = self
            .worker_tx
            .send(WorkerMsg::QueryFocusedState { reply: tx });
        rx.recv()
    }

    fn focused_element(&self) -> Option<ElementNodeId> {
        let (tx, rx) = crate::core::app::Reply::<Option<ElementNodeId>>::pair();
        let _ = self
            .worker_tx
            .send(WorkerMsg::QueryFocusedElement { reply: tx });
        rx.recv()
    }

    fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let (tx, rx) = crate::core::app::Reply::<Option<(f64, f64, f64, f64)>>::pair();
        let _ = self
            .worker_tx
            .send(WorkerMsg::QueryFocusedCursorRect { reply: tx });
        rx.recv()
    }

    fn focused_is_editable(&self) -> bool {
        let (tx, rx) = crate::core::app::Reply::<bool>::pair();
        let _ = self
            .worker_tx
            .send(WorkerMsg::QueryFocusedIsEditable { reply: tx });
        rx.recv()
    }

    fn query_element(&self, key: &[&str]) -> Option<NodeId> {
        let (tx, rx) = crate::core::app::Reply::<Option<NodeId>>::pair();
        let key_owned: Vec<String> = key.iter().map(|s| s.to_string()).collect();
        let _ = self.worker_tx.send(WorkerMsg::QueryElement {
            key: key_owned,
            reply: tx,
        });
        rx.recv()
    }

    fn dev_tool_element_tree(&self) -> Option<DevNodeData> {
        // RPC via DevElementTree.
        let (tx, rx) = crate::core::app::Reply::<Option<DevNodeData>>::pair();
        let _ = self.worker_tx.send(WorkerMsg::DevElementTree { reply: tx });
        rx.recv()
    }

    fn dev_tool_get_element(&self, id: NodeId) -> Option<DevNodeData> {
        let (tx, rx) = crate::core::app::Reply::<Option<DevNodeData>>::pair();
        let _ = self
            .worker_tx
            .send(WorkerMsg::DevGetElement { id, reply: tx });
        rx.recv()
    }

    fn render_to_pixels(&self) -> Option<Vec<u8>> {
        let (tx, rx) = crate::core::app::Reply::<Option<Vec<u8>>>::pair();
        let _ = self.worker_tx.send(WorkerMsg::RenderToPixels { reply: tx });
        rx.recv()
    }

    fn set_cursor_backend(&self, backend: Rc<RefCell<dyn CursorBackend>>) {
        // Store on main. Worker emits `MainMsg::CursorChanged` during
        // pump; main applies here.
        *self.cursor_backend.borrow_mut() = Some(backend);
    }

    fn with_boa_context_dyn(&self, _f: BoaClosure) -> AnySend {
        unimplemented!("with_boa_context not supported in threaded mode (use RPC variants)")
    }

    fn with_element_dyn(&self, _id: ElementNodeId, _f: ElementClosure) -> Option<AnySend> {
        unimplemented!("with_element not supported in threaded mode (use dev_tool_* RPC)")
    }

    #[cfg(feature = "trace")]
    fn element_tree_handle(&self) -> std::cell::Ref<'_, crate::core::elements::NodeTreeData> {
        unimplemented!("element_tree not supported in threaded mode")
    }
}
